# Nexus 测试计划 — Part 3

> 续 [NEXUS_TEST_PLAN_PART2.md](./NEXUS_TEST_PLAN_PART2.md)

---

## 14. Escrow — 托管模块

> Pallet: `pallet-escrow` | Extrinsics: 12 (call_index 0-11)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| ES-001 | lock：锁定资金 | 授权模块 | 正向 | P0 |
| ES-002 | lock_with_nonce：幂等锁定（nonce 严格递增） | 授权模块 | 正向 | P1 |
| ES-003 | release：释放资金给收款人 | 授权模块 | 正向 | P0 |
| ES-004 | refund：退款给付款人 | 授权模块 | 正向 | P0 |
| ES-005 | release_split：分账释放（多方，合计 ≤ 托管余额） | 授权模块 | 正向 | P1 |
| ES-006 | 已关闭 ID 重复锁定被拒绝（AlreadyClosed，审计 EH3） | 授权模块 | 负向 | P1 |
| ES-007 | dispute：进入争议状态 | 授权模块 | 正向 | P1 |
| ES-008 | 争议状态下普通释放/退款被拒绝（DisputeActive，审计 EM2） | 授权模块 | 安全 | P0 |
| ES-009 | apply_decision_release_all / refund_all / partial_bps：仲裁三种模式 | R1/R11 | 正向 | P1 |
| ES-010 | set_pause：全局暂停时所有操作被拒绝 | R1 | 安全 | P1 |
| ES-011 | schedule_expiry / cancel_expiry：安排/取消到期处理 | 授权模块 | 正向 | P2 |
| ES-012 | 到期队列满返回 ExpiringAtFull（审计 EM1） | 授权模块 | 负向 | P2 |

## 15. Evidence — 证据模块

> Pallet: `pallet-evidence` | Extrinsics: 13 (call_index 0-12)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| EV-001 | commit：提交证据（图片/视频/文档 CID） | R5/R6 | 正向 | P0 |
| EV-002 | 超过 MaxImg/MaxVid/MaxDoc 被拒绝（审计 VH1） | R5/R6 | 负向 | P1 |
| EV-003 | commit_hash：提交哈希承诺 | R5/R6 | 正向 | P1 |
| EV-004 | append_evidence：追加证据（bounds 验证，审计 VH2） | R5/R6 | 正向 | P1 |
| EV-005 | update_evidence_manifest：修改待处理证据（编辑窗口内） | R5/R6 | 正向 | P1 |
| EV-006 | link / unlink：链接/取消链接证据到目标 | 授权模块 | 正向 | P1 |
| EV-007 | link_by_ns / unlink_by_ns：按命名空间链接 | 授权模块 | 正向 | P2 |
| EV-008 | register_public_key：注册用户公钥 | R20 | 正向 | P1 |
| EV-009 | store_private_content：存储私密内容 | R20 | 正向 | P1 |
| EV-010 | grant_access / revoke_access：授予/撤销访问权限 | R20 | 正向 | P1 |
| EV-011 | rotate_content_keys：轮换加密密钥（O(1) 计数器，审计 VM1） | R20 | 正向 | P2 |

## 16. Arbitration — 仲裁模块

> Pallet: `pallet-arbitration` | Extrinsics: 12 (call_index 0-5, 10-15)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AR-001 | dispute：发起仲裁 | R5/R6 | 正向 | P0 |
| AR-002 | dispute_with_evidence_id：带 evidence_id 发起 | R5/R6 | 正向 | P1 |
| AR-003 | append_evidence_id：补充证据（审计 AH1: auth 检查） | R5/R6 | 正向 | P1 |
| AR-004 | arbitrate：仲裁裁决（Root/委员会） | R1/R11 | 正向 | P0 |
| AR-005 | 非授权无法裁决 | R20 | 权限 | P0 |
| AR-006 | dispute_with_two_way_deposit：双方押金仲裁 | R5 | 正向 | P1 |
| AR-007 | respond_to_dispute：应诉方响应 | R6 | 正向 | P1 |
| AR-008 | file_complaint：发起投诉（缴押金） | R5/R6 | 正向 | P0 |
| AR-009 | respond_to_complaint：响应投诉 | R5/R6 | 正向 | P0 |
| AR-010 | withdraw_complaint：撤销投诉（审计 AH5: 退押金） | R5/R6 | 正向 | P1 |
| AR-011 | settle_complaint：达成和解（审计 AH6: 押金退还） | R5/R6 | 正向 | P1 |
| AR-012 | escalate_to_arbitration：升级到仲裁 | R5/R6 | 正向 | P0 |
| AR-013 | resolve_complaint：仲裁裁决投诉 | R1/R11 | 正向 | P0 |
| AR-014 | 过期投诉处理（ComplaintExpiryCursor 游标，审计 AH4） | R20 | 功能 | P2 |

## 17. Storage Service — 存储服务

> Pallet: `pallet-storage-service` | Extrinsics: 24 (call_index 1-23)

### 17.1 用户操作

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-001 | fund_user_account：充值用户账户 | R20 | 正向 | P0 |
| SS-002 | request_pin_for_subject：请求 Pin 文件 | R20 | 正向 | P0 |
| SS-003 | 余额不足 Pin 被拒绝 | R20 | 负向 | P0 |

### 17.2 运营者管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-004 | join_operator：加入运营者（存保证金） | R14 | 正向 | P0 |
| SS-005 | 容量 < MinCapacityGiB 被拒绝 | R14 | 负向 | P1 |
| SS-006 | 保证金 < MinOperatorBond 被拒绝 | R14 | 负向 | P1 |
| SS-007 | update_operator：更新运营者元信息 | R14 | 正向 | P1 |
| SS-008 | leave_operator：退出运营者（退保证金） | R14 | 正向 | P1 |
| SS-009 | pause_operator / resume_operator：暂停/恢复 | R14 | 正向 | P1 |
| SS-010 | set_operator_status：治理设置状态 | R1 | 正向 | P1 |
| SS-011 | report_probe：运营者自证在线 | R14 | 正向 | P1 |
| SS-012 | operator_claim_rewards：领取奖励 | R14 | 正向 | P1 |

### 17.3 OCW 操作

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-013 | mark_pinned：标记 Pin 成功 | R19 | 正向 | P0 |
| SS-014 | mark_pin_failed：标记 Pin 失败 | R19 | 正向 | P1 |

### 17.4 治理操作

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SS-015 | charge_due：处理到期扣费 | R1 | 正向 | P0 |
| SS-016 | 余额不足 → 宽限 → 过期 | 系统 | 流程 | P1 |
| SS-017 | set_billing_params：设置计费参数 | R1 | 正向 | P1 |
| SS-018 | distribute_to_operators：分配收益 | R1 | 正向 | P1 |
| SS-019 | set_replicas_config：设置副本配置 | R1 | 正向 | P2 |
| SS-020 | update_tier_config：更新层级配置 | R1 | 正向 | P2 |
| SS-021 | set_operator_layer：设置运营者层级 | R1 | 正向 | P2 |
| SS-022 | set_storage_layer_config：设置存储层配置 | R1 | 正向 | P2 |
| SS-023 | slash_operator：扣罚保证金 | R1 | 正向 | P1 |
| SS-024 | emergency_pause_billing：紧急暂停计费 | R1 | 安全 | P1 |

## 18. GroupRobot Registry — Bot 注册管理

> Pallet: `pallet-grouprobot-registry` | Extrinsics: 30 (call_index 0-30)

### 18.1 Bot 生命周期

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-001 | register_bot：注册 Bot | R12 | 正向 | P0 |
| GR-002 | update_public_key：更换 Bot 公钥（密钥轮换） | R12 | 正向 | P1 |
| GR-003 | deactivate_bot：停用 Bot（清理 Attestations/Peers/Operators） | R12 | 正向 | P1 |
| GR-004 | bind_community / unbind_community：绑定/解绑社区 | R12 | 正向 | P0 |
| GR-005 | bind_user_platform：用户绑定平台身份 | R20 | 正向 | P1 |

### 18.2 TEE 证明 — 软件模式

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-006 | submit_attestation：提交 TEE 双证明（is_simulated=true） | R12 | 正向 | P0 |
| GR-007 | refresh_attestation：刷新 TEE 证明（24h 周期） | R12 | 正向 | P1 |

### 18.3 TEE 证明 — DCAP 硬件模式

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-008 | approve_mrtd / approve_mrenclave：审批白名单 | R1 | 正向 | P0 |
| GR-009 | revoke_mrtd / revoke_mrenclave：撤销白名单 | R1 | 正向 | P1 |
| GR-010 | request_attestation_nonce：请求证明 nonce | R12 | 正向 | P0 |
| GR-011 | submit_verified_attestation：提交已验证 TDX 证明 | R12 | 正向 | P0 |
| GR-012 | submit_dcap_attestation：提交 DCAP 单 Quote（Level 2/3） | R12 | 正向 | P0 |
| GR-013 | submit_dcap_dual_attestation：提交双 Quote（Bot + API Server） | R12 | 正向 | P0 |
| GR-014 | submit_dcap_full_attestation：完整证书链（Level 4） | R12 | 正向 | P1 |
| GR-015 | submit_sgx_attestation：提交 SGX 证明 | R12 | 正向 | P1 |
| GR-016 | submit_tee_attestation：统一 TEE 证明入口 | R12 | 正向 | P1 |
| GR-017 | 篡改 MRTD → body 签名无效 → 拒绝 | R12 | 安全 | P0 |
| GR-018 | 篡改 report_data → 签名无效 → 拒绝 | R12 | 安全 | P0 |
| GR-019 | 伪造 Quote → 无合法 ECDSA 签名 → 拒绝 | R12 | 安全 | P0 |
| GR-020 | 错误 PCK → QE Report 签名失败 → 拒绝（Level 3） | R12 | 安全 | P1 |
| GR-021 | 双 Quote 必须绑定同一 bot.public_key | R12 | 安全 | P0 |
| GR-022 | approve_api_server_mrtd：审批 API Server MRTD | R1 | 正向 | P1 |
| GR-023 | register_pck_key：注册 PCK Key | R1 | 正向 | P1 |

### 18.4 Peer 网络

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-024 | register_peer：注册 Peer（需 TEE + 付费订阅） | R12 | 正向 | P0 |
| GR-025 | Free tier 注册被拒绝（FreeTierNotAllowed） | R12 | 安全 | P0 |
| GR-026 | deregister_peer：注销 Peer | R12 | 正向 | P1 |
| GR-027 | heartbeat_peer：心跳（更新 last_seen + 付费检查） | R12 | 正向 | P0 |
| GR-028 | report_stale_peer：举报过期 Peer | R20 | 正向 | P1 |

### 18.5 运营商管理

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GR-029 | register_operator：注册运营商（platform_app_hash 唯一） | R18 | 正向 | P1 |
| GR-030 | 同平台 platform_app_hash 重复被拒绝 | R18 | 负向 | P1 |
| GR-031 | update_operator / deregister_operator：更新/注销 | R18 | 正向 | P1 |
| GR-032 | set_operator_sla：设置 SLA 等级（Root） | R1 | 正向 | P2 |
| GR-033 | assign_bot_to_operator：分配 Bot 到运营商 | R12 | 正向 | P1 |

## 19. GroupRobot Consensus — 节点共识

> Pallet: `pallet-grouprobot-consensus` | Extrinsics: 8 (call_index 0-4, 10-12)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CN-001 | register_node：注册节点 + 质押 | R13 | 正向 | P0 |
| CN-002 | 质押不足被拒绝 | R13 | 负向 | P0 |
| CN-003 | request_exit：申请退出（冷却期） | R13 | 正向 | P1 |
| CN-004 | finalize_exit：完成退出 + 退质押（审计 H3: 先领孤儿奖励） | R13 | 正向 | P1 |
| CN-005 | 冷却期未到无法完成退出 | R13 | 负向 | P1 |
| CN-006 | report_equivocation：举报 Equivocation | R13 | 正向 | P1 |
| CN-007 | slash_equivocation：执行 Slash（Root） | R1 | 正向 | P1 |
| CN-008 | mark_sequence_processed：标记序列已处理（去重） | R13 | 正向 | P0 |
| CN-009 | Free tier 标记被拒绝（FreeTierNotAllowed） | R13 | 安全 | P0 |
| CN-010 | verify_node_tee：验证节点 TEE | R13 | 正向 | P1 |
| CN-011 | set_tee_reward_params：设置 TEE 奖励参数（Root） | R1 | 正向 | P2 |

## 20. GroupRobot Subscription — 订阅服务

> Pallet: `pallet-grouprobot-subscription` | Extrinsics: 6 (call_index 0-5)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SB-001 | subscribe：订阅 Bot 服务（Basic/Pro/Enterprise） | R17 | 正向 | P0 |
| SB-002 | deposit_subscription：充值续期 | R17 | 正向 | P1 |
| SB-003 | cancel_subscription：取消订阅 | R17 | 正向 | P1 |
| SB-004 | change_tier：变更层级 | R17 | 正向 | P1 |
| SB-005 | commit_ads：广告承诺订阅 | R17 | 正向 | P1 |
| SB-006 | cancel_ad_commitment：取消广告承诺 | R17 | 正向 | P2 |
| SB-007 | effective_tier 返回正确层级（is_paid() 检查） | 系统 | 功能 | P0 |
| SB-008 | 过期订阅降级为 Free | 系统 | 功能 | P1 |

## 21. GroupRobot Community — 社区管理

> Pallet: `pallet-grouprobot-community` | Extrinsics: 11 (call_index 0-10)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GC-001 | submit_action_log：提交动作日志（Ed25519 签名验证） | R12 | 正向 | P0 |
| GC-002 | Free tier 提交被拒绝（FreeTierNotAllowed） | R12 | 安全 | P0 |
| GC-003 | 签名无效被拒绝 | R12 | 安全 | P0 |
| GC-004 | 序列号重复被拒绝（去重） | R12 | 负向 | P1 |
| GC-005 | batch_submit_logs：批量提交（每条独立验证） | R12 | 正向 | P1 |
| GC-006 | set_node_requirement：设置节点准入策略 | R15 | 正向 | P1 |
| GC-007 | update_community_config：更新配置（CAS 乐观锁） | R15 | 正向 | P1 |
| GC-008 | 版本号不匹配被拒绝（CAS 冲突） | R15 | 负向 | P1 |
| GC-009 | reward_reputation / deduct_reputation / reset_reputation：声誉管理（Bot owner） | R12 | 正向 | P1 |
| GC-010 | 非 Bot owner 操作声誉被拒绝 | R20 | 权限 | P1 |
| GC-011 | update_active_members：更新活跃成员数 | R12 | 正向 | P1 |
| GC-012 | clear_expired_logs：清理过期日志（max_age_blocks > 0） | R20 | 正向 | P1 |
| GC-013 | max_age_blocks = 0 被拒绝（防擦除全部） | R20 | 安全 | P1 |
| GC-014 | 付费: log_retention_days > 0 时强制最小间隔（days × 14400） | 系统 | 功能 | P1 |
| GC-015 | Enterprise（retention_days=0）：拒绝 cleanup | 系统 | 功能 | P2 |

## 22. GroupRobot Ceremony — 密钥仪式

> Pallet: `pallet-grouprobot-ceremony` | Extrinsics: 5 (call_index 0-4)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| CE-001 | record_ceremony：记录仪式（Shamir 参数 + Enclave 白名单） | R12 | 正向 | P0 |
| CE-002 | participant_count < k 被拒绝（InsufficientParticipants，审计 H1） | R12 | 安全 | P0 |
| CE-003 | Free tier 被拒绝（FreeTierNotAllowed） | R12 | 安全 | P0 |
| CE-004 | bot_id_hash 正确存储（非 blake2_256 派生，审计 C1） | R12 | 安全 | P0 |
| CE-005 | revoke_ceremony：撤销仪式（Root） | R1 | 正向 | P1 |
| CE-006 | approve_ceremony_enclave / remove_ceremony_enclave：白名单管理 | R1 | 正向 | P1 |
| CE-007 | force_re_ceremony：强制 re-ceremony（仅 Active 状态，审计 H2） | R1 | 正向 | P1 |
| CE-008 | 已撤销/过期仪式不可 re-ceremony（CeremonyNotActive） | R1 | 负向 | P1 |
| CE-009 | approve_ceremony_enclave 描述超长被拒绝（DescriptionTooLong，审计 M1） | R1 | 负向 | P2 |
| CE-010 | CeremonyAtRisk 检测（on_initialize 使用 record.bot_id_hash） | 系统 | 功能 | P1 |

## 23. GroupRobot Rewards — 节点奖励

> Pallet: `pallet-grouprobot-rewards` | Extrinsics: 3 (call_index 0-2)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| RW-001 | claim_rewards：领取节点奖励（审计 H2: 先转账后清存储） | R13 | 正向 | P0 |
| RW-002 | 无可领奖励被拒绝 | R13 | 负向 | P1 |
| RW-003 | set_reward_params：设置奖励参数（Root） | R1 | 正向 | P1 |
| RW-004 | prune_old_era_rewards：清理旧 Era 奖励（审计 H1: 有界循环 MAX=10） | R1 | 正向 | P2 |
| RW-005 | TEE 节点奖励倍数正确应用 | 系统 | 功能 | P1 |
| RW-006 | finalize_exit 前尝试领取孤儿奖励（审计 H3: OrphanRewardClaimer） | 系统 | 功能 | P1 |
| RW-007 | 奖励池不足时返回 RewardPoolInsufficient | 系统 | 负向 | P2 |

## 24. GroupRobot Ads — 广告系统

> Pallet: `pallet-grouprobot-ads` | Extrinsics: 21 (call_index 0-20)

### 24.1 Campaign 生命周期

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-001 | create_campaign：创建广告活动（锁预算到 escrow） | R16 | 正向 | P0 |
| AD-002 | fund_campaign：追加预算 | R16 | 正向 | P1 |
| AD-003 | pause_campaign：暂停广告 | R16 | 正向 | P1 |
| AD-004 | cancel_campaign：取消广告（退还剩余） | R16 | 正向 | P1 |
| AD-005 | review_campaign：审核广告内容（Root/DAO） | R1 | 正向 | P1 |
| AD-006 | flag_campaign：举报广告 | R20 | 正向 | P2 |

### 24.2 投放与结算

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-007 | submit_delivery_receipt：提交投放收据（audience_cap 裁切） | R12 | 正向 | P0 |
| AD-008 | audience_size 超过 cap 被裁切（不拒绝） | R12 | 功能 | P0 |
| AD-009 | settle_era_ads：Era 结算（CPM 计费） | R20 | 正向 | P0 |
| AD-010 | claim_ad_revenue：社区提取广告收入 | R15 | 正向 | P1 |

### 24.3 质押与 Audience Cap

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-011 | stake_for_ads：质押获取 audience_cap | R15 | 正向 | P0 |
| AD-012 | unstake_from_ads：取消质押降低上限 | R15 | 正向 | P1 |

### 24.4 双向偏好

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-013 | advertiser_block_community / advertiser_unblock_community：广告主拉黑/取消 | R16 | 正向 | P1 |
| AD-014 | advertiser_whitelist_community：广告主指定白名单 | R16 | 正向 | P2 |
| AD-015 | community_block_advertiser / community_unblock_advertiser：社区拉黑/取消 | R15 | 正向 | P1 |
| AD-016 | community_whitelist_advertiser：社区指定白名单 | R15 | 正向 | P2 |
| AD-017 | 非管理员操作社区偏好被拒绝 | R20 | 权限 | P1 |

### 24.5 Slash 与反作弊

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-018 | slash_community：扣质押 + cap 砍半（Root） | R1 | 正向 | P0 |
| AD-019 | 连续 3 次 Slash → 永久封禁 | R1 | 功能 | P1 |
| AD-020 | flag_community：举报社区作弊 | R20 | 正向 | P1 |
| AD-021 | report_node_audience：节点上报 audience 统计（多节点交叉验证） | R12 | 正向 | P1 |
| AD-022 | check_audience_surge：audience 突增检测（仅 TEE 节点） | R12 | 正向 | P1 |
| AD-023 | 突增超阈值 → 自动暂停 2 个 Era | 系统 | 功能 | P1 |
| AD-024 | Era 结算时多节点偏差验证 | 系统 | 功能 | P2 |

### 24.6 治理参数

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| AD-025 | set_tee_ad_share：设置 TEE 节点广告分成（community+tee ≤ 100） | R1 | 正向 | P1 |
| AD-026 | set_community_ad_share：设置社区分成（community ≥ 50） | R1 | 正向 | P1 |
| AD-027 | set_community_admin：设置/变更社区管理员 | R1 | 正向 | P1 |

---

## 25. 跨模块集成测试

### 25.1 完整商业流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-001 | 创建 Entity → 审批 → 创建 Shop → 创建 Product → 上架 → 下单 → 发货 → 确认 → 评价 | Registry+Shop+Service+Order+Review | P0 |
| INT-002 | 下单 → 托管锁资金 → 确认收货 → 释放资金 → 佣金分配 → 会员升级 | Order+Escrow+Commission+Member | P0 |
| INT-003 | 注册会员 → 绑定推荐人 → 下单 → 触发 Referral 佣金 → 多级分发 → 提现 | Member+Order+Commission | P0 |
| INT-004 | 创建 Entity Token → 配置治理 → 创建提案 → 投票 → 执行（on-chain 生效） | Token+Governance | P1 |
| INT-005 | 创建 TokenSale → 认购 → 结束 → 领取 Token → 二级市场交易 | TokenSale+Token+EntityMarket | P1 |
| INT-006 | KYC 认证 → 满足 Entity 要求 → 参与 Token 转账（KycRequired 模式） | KYC+Token | P1 |
| INT-007 | 订单争议 → 提交证据 → 发起投诉 → 仲裁 → 释放托管 | Order+Evidence+Arbitration+Escrow | P0 |
| INT-008 | Entity 推荐人绑定 → 下单时平台费分成 → 推荐人收益 | Registry+Order+Commission | P1 |
| INT-009 | Token 订单 → Token 佣金分配 → Token 提现（独立提现配置） | Order+Commission(Token) | P1 |

### 25.2 GroupRobot 完整流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-010 | 注册 Bot → 订阅 → DCAP 证明 → 注册 Peer → 心跳 → Ceremony | Registry+Subscription+Ceremony | P0 |
| INT-011 | 注册节点 → 质押 → 验证 TEE → 处理消息 → 领取奖励 | Consensus+Registry+Rewards | P0 |
| INT-012 | 订阅 → 绑定社区 → 提交动作日志 → 声誉管理 → 清理过期 | Subscription+Community | P1 |
| INT-013 | 创建广告 → 质押 audience → 投放 → 结算 → 提取收入 | Ads+Subscription | P0 |
| INT-014 | Tier 门控: Free tier Bot 无法注册 Peer/提交日志/处理消息/记录仪式 | Subscription+Registry+Community+Consensus+Ceremony | P0 |

### 25.3 存储完整流程

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-015 | 充值 → 请求 Pin → OCW Pin → mark_pinned → 到期扣费 → 续期 | Storage-Service | P0 |
| INT-016 | 运营者加入 → 自证在线 → 分配收益 → 领取奖励 → 退出 | Storage-Service | P1 |
| INT-017 | 余额不足 → 宽限期 → 到期 → 自动清理 | Storage-Service | P1 |

### 25.4 审计回归集成

| # | 测试用例 | 涉及模块 | 优先级 |
|---|---------|----------|--------|
| INT-018 | 治理提案执行会员等级（shop_id → entity_id 正确解析，审计 C2） | Governance+Member(Runtime Bridge) | P0 |
| INT-019 | cancel_commission 转账失败保持 Pending（审计 H2） | Commission+Order | P0 |
| INT-020 | USDT 交易 verify_usdt_payment 正确释放买家保证金（审计 C1） | EntityMarket | P0 |

---

## 26. 端到端流程测试（多用户）

| # | 场景 | 参与角色 | 优先级 |
|---|------|----------|--------|
| E2E-001 | **商家入驻到交易全流程**: Root 审批 Entity → Owner 建 Shop/Product → Buyer 下单支付 → Seller 发货 → Buyer 确认 → 佣金结算 → Member 升级 | R1,R2,R4,R5,R6,R7 | P0 |
| E2E-002 | **会员推荐裂变**: MemberA 推荐 B → B 推荐 C → C 下单 → 三级 Referral 佣金分发 → 各级提现 | R5,R7 | P0 |
| E2E-003 | **争议解决全流程**: 下单 → 发货 → 买家申请退款 → 卖家拒绝 → 买家提交证据 → 发起投诉 → 仲裁裁决 → 托管按比例分配 | R5,R6,R11 | P0 |
| E2E-004 | **代币发售到治理**: 创建 Token → 发售 → 认购 → 结束 → 领取 Token → 创建提案 → 投票 → 执行 | R2,R5,R8 | P1 |
| E2E-005 | **Bot 运营全链路**: 注册 Bot → 付费订阅 → DCAP 证明 → 注册 Peer → 注册节点 → 质押 → 绑定社区 → 提交日志 → 领取奖励 | R12,R13,R17 | P0 |
| E2E-006 | **广告投放与反作弊**: 广告主创建 Campaign → 社区质押 → Bot 投放 → Era 结算 → 社区提取 → 异常检测 → Slash | R12,R15,R16 | P1 |
| E2E-007 | **P2P 交易全流程**: Seller 挂卖单 → Buyer 吃单 + 付款 → OCW 验证 → 多档判定 → 补付 → 结算 → TWAP 更新 | R9,R19 | P0 |
| E2E-008 | **存储服务全流程**: 用户充值 → Pin 文件 → 运营者 Pin 成功 → 定期扣费 → 续期 → 运营者领取奖励 | R14,R20 | P1 |
| E2E-009 | **密钥仪式与迁移**: 记录仪式 → Active → force_re_ceremony → 新仪式 → 旧仪式 superseded | R1,R12 | P1 |
| E2E-010 | **Entity Token 二级市场**: 创建 Token → TokenSale → Entity Market 挂单 → NEX 对价成交 + USDT 对价成交 | R2,R8,R19 | P1 |

---

## 27. 性能与边界测试

| # | 测试用例 | 优先级 |
|---|---------|--------|
| PF-001 | Entity 达到 MaxShopsPerEntity(16) 上限 | P2 |
| PF-002 | Shop 达到 MaxProductsPerShop(1000) 上限 | P2 |
| PF-003 | 单 Entity 最大 Admin 数(10) | P2 |
| PF-004 | 治理 MaxActiveProposals(10) 并发 | P2 |
| PF-005 | Member MaxCustomLevels(10) 等级 | P2 |
| PF-006 | TokenSale MaxSubscriptionsPerRound(10000) 大量认购 | P2 |
| PF-007 | Disclosure MaxInsiders(50) | P2 |
| PF-008 | NexMarket 大量并发挂单撮合 | P1 |
| PF-009 | Community 大量日志批量提交 | P2 |
| PF-010 | Storage charge_due 大量到期 CID 处理 | P2 |
| PF-011 | EntityMarket OrderBook 深度 + market_buy/market_sell 遍历（O(N) 性能） | P2 |

---

## 28. 安全性测试

| # | 测试用例 | 优先级 |
|---|---------|--------|
| SEC-001 | 所有 extrinsic 权限校验（非授权用户被拒绝） | P0 |
| SEC-002 | 资金操作：不可取出超过可用余额 | P0 |
| SEC-003 | 双花防护：托管资金不可被多次释放 | P0 |
| SEC-004 | 重放保护：nonce 递增 / 序列号去重 / tx_hash 防重放 | P0 |
| SEC-005 | 溢出保护：金额计算使用 saturating_add / checked_mul | P0 |
| SEC-006 | TEE 证明伪造检测（签名验证） | P0 |
| SEC-007 | TWAP 价格操纵保护（最大偏离阈值 + 熔断） | P0 |
| SEC-008 | KYC 过期后权限自动降级 | P1 |
| SEC-009 | 佣金保护：Entity 资金 ≥ 已承诺金额 | P0 |
| SEC-010 | 广告反作弊：audience_cap 强制裁切 + 多节点交叉验证 | P1 |
| SEC-011 | Free tier 门控：所有付费功能正确拒绝 | P0 |
| SEC-012 | 等级过期时佣金/折扣正确回退（审计 H6） | P1 |
| SEC-013 | 购物余额不可提取为 NEX | P0 |
| SEC-014 | Escrow 争议状态下禁止常规释放/退款 | P0 |
| SEC-015 | Entity 封禁后关联 Shop/Product 不可交易 | P1 |
| SEC-016 | validate_unsigned 拒绝 External 来源（审计 H1: market） | P0 |
| SEC-017 | SingleLine 佣金使用 order_amount 而非累计（审计 C2） | P0 |

---

## 统计

| 模块 | 测试用例数 |
|------|:---------:|
| Entity Registry | 25 |
| Entity Shop | 14 |
| Entity Service | 8 |
| Entity Order | 15 |
| Entity Review | 4 |
| Entity Token | 14 |
| Entity Governance | 12 |
| Entity Member | 28 |
| Commission Core | 14 |
| Commission 分配 | 10 |
| Commission 插件 | 12 |
| Entity Disclosure | 9 |
| Entity KYC | 14 |
| Entity TokenSale | 19 |
| NEX Market | 30 |
| Entity Market | 13 |
| Escrow | 12 |
| Evidence | 11 |
| Arbitration | 14 |
| Storage Service | 24 |
| GR Registry | 33 |
| GR Consensus | 11 |
| GR Subscription | 8 |
| GR Community | 15 |
| GR Ceremony | 10 |
| GR Rewards | 7 |
| GR Ads | 27 |
| 跨模块集成 | 20 |
| 端到端 | 10 |
| 性能/边界 | 11 |
| 安全性 | 17 |
| **合计** | **~485** |
