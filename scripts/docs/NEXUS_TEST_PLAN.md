# Nexus 项目全面测试计划

> 基于所有用户角色的功能性、流程性测试清单
> 生成日期: 2026-03-02 | 覆盖: 37 链上 Pallet + GroupRobot 链下模块
> 基于最新代码自动生成，包含审计修复后的回归测试点

---

## 用户角色清单

| # | 角色 | 描述 |
|---|------|------|
| R1 | **Root/Sudo** | 超级管理员（GovernanceOrigin） |
| R2 | **Entity Owner** | 实体创建者/所有者 |
| R3 | **Entity Admin** | 实体管理员（≤10人） |
| R4 | **Shop Manager** | 店铺管理员 |
| R5 | **Buyer/Consumer** | 买家/消费者 |
| R6 | **Seller/Merchant** | 卖家/服务商 |
| R7 | **Member** | 注册会员 |
| R8 | **Token Holder** | 代币持有者 |
| R9 | **NEX Trader** | P2P 交易者 |
| R10 | **KYC Provider** | KYC 认证机构 |
| R11 | **Arbitrator/Committee** | 仲裁委员 |
| R12 | **Bot Owner** | Bot 所有者 |
| R13 | **Node Operator** | 共识节点运营者 |
| R14 | **Storage Operator** | 存储运营者 |
| R15 | **Community Admin** | 社区管理员 |
| R16 | **Advertiser** | 广告主 |
| R17 | **Subscriber** | Bot 订阅用户 |
| R18 | **Bot Operator** | Bot 运营商 |
| R19 | **OCW** | 链下工作机 |
| R20 | **普通用户** | 无特殊权限 |

---

## 1. Entity Registry — 实体注册管理

> Pallet: `pallet-entity-registry` | Extrinsics: 16 (call_index 0-16, 13已移除)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| ER-001 | create_entity(Merchant)：付费即激活 Active，50 USDT 等值 NEX 押金，自动创建 Primary Shop | R2 | 正向 | P0 |
| ER-002 | 创建不同 EntityType（Enterprise/DAO/Community/Project/Fund/Custom） | R2 | 正向 | P1 |
| ER-003 | 创建实体时绑定推荐人（referrer 参数） | R2 | 正向 | P1 |
| ER-004 | 名称为空/超过 BoundedVec 限长被拒绝 | R2 | 负向 | P1 |
| ER-005 | 余额不足支付初始资金被拒绝 | R2 | 负向 | P0 |
| ER-006 | 超过 MaxEntitiesPerUser(3) 被拒绝 | R2 | 负向 | P1 |
| ER-007 | 价格不可用时创建被拒绝 | R2 | 负向 | P1 |
| ER-008 | update_entity：Owner/Admin 更新实体名称和 logo | R2/R3 | 正向 | P1 |
| ER-009 | 非 Owner/Admin 无法更新 | R20 | 权限 | P1 |
| ER-010 | request_close_entity：Owner 申请关闭→PendingClose | R2 | 正向 | P1 |
| ER-011 | top_up_fund：任何人充值金库 | R20 | 正向 | P1 |
| ER-012 | approve_entity：治理审批 reopen 后的 Pending→Active，恢复 Shop | R1 | 正向 | P0 |
| ER-013 | 非治理 Origin 无法审批 | R20 | 权限 | P0 |
| ER-014 | approve_close_entity：治理审批关闭→退还全部余额 | R1 | 正向 | P1 |
| ER-015 | suspend_entity：治理暂停实体 | R1 | 正向 | P1 |
| ER-016 | resume_entity：治理恢复实体（需资金充足） | R1 | 正向 | P1 |
| ER-017 | 恢复时资金不足被拒绝 | R1 | 负向 | P1 |
| ER-018 | ban_entity：治理封禁实体（可选没收资金） | R1 | 正向 | P1 |
| ER-019 | add_admin/remove_admin：添加/移除管理员，超过 10 人被拒绝 | R2 | 正向+负向 | P1 |
| ER-020 | transfer_ownership：转移所有权 | R2 | 正向 | P1 |
| ER-021 | upgrade_entity_type：升级实体类型 | R1 | 正向 | P2 |
| ER-022 | verify_entity：验证实体 | R1 | 正向 | P2 |
| ER-023 | reopen_entity：Owner 重开已关闭实体→Pending→需 approve_entity | R2 | 正向 | P2 |
| ER-024 | Banned 状态实体不可 reopen | R2 | 负向 | P2 |
| ER-025 | bind_entity_referrer：绑定推荐人/不能推荐自己/已绑定不可重复 | R2 | 正向+负向 | P2 |
| ER-026 | 资金健康状态检测 | R20 | 功能 | P2 |

## 2. Entity Shop — 店铺管理

> Pallet: `pallet-entity-shop` | Extrinsics: 14 (call_index 0-13)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SH-001 | create_shop：关联已激活实体创建店铺 | R2/R3 | 正向 | P0 |
| SH-002 | 实体未激活时创建被拒绝 | R2 | 负向 | P0 |
| SH-003 | 超过 MaxShopsPerEntity(16) 被拒绝 | R2 | 负向 | P1 |
| SH-004 | update_shop：更新名称/logo/描述 | R4 | 正向 | P1 |
| SH-005 | 空 CID (logo_cid/description_cid) 被拒绝 (EmptyCid) | R4 | 负向 | P1 |
| SH-006 | set_location：设置位置/营业时间，空 address_cid 被拒绝 | R4 | 正向+负向 | P1 |
| SH-007 | 非授权账户更新被拒绝 | R20 | 权限 | P1 |
| SH-008 | add_manager/remove_manager：添加/移除店铺管理员 | R2 | 正向 | P1 |
| SH-009 | fund_operating：充值运营资金，零金额被拒绝 (ZeroFundAmount) | R2/R4 | 正向+负向 | P1 |
| SH-010 | withdraw_operating_fund：提取运营资金 | R2 | 正向 | P1 |
| SH-011 | 提取超过可用余额(扣除 pending+shopping 佣金保护)被拒绝 | R2 | 安全 | P0 |
| SH-012 | pause_shop/resume_shop：暂停/恢复店铺 | R2/R4 | 正向 | P1 |
| SH-013 | 已关闭 Shop 不可 pause/resume (状态校验) | R2 | 负向 | P1 |
| SH-014 | close_shop：关闭店铺（调用 unregister_shop） | R2 | 正向 | P1 |
| SH-015 | enable_points/disable_points：启用/禁用积分系统 | R2 | 正向 | P1 |
| SH-016 | update_points_config：更新积分配置（已关闭店铺被拒绝） | R2 | 正向+负向 | P2 |
| SH-017 | transfer_points：用户间转移积分（已关闭店铺被拒绝） | R20 | 正向+负向 | P1 |
| SH-018 | 积分余额不足转移被拒绝 | R20 | 负向 | P1 |

## 3. Entity Service — 商品/服务管理

> Pallet: `pallet-entity-service` | Extrinsics: 5 (call_index 0-4)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SV-001 | create_product：创建商品（从店铺派生账户扣取押金） | R4/R6 | 正向 | P0 |
| SV-002 | update_product：更新商品信息 | R4/R6 | 正向 | P1 |
| SV-003 | publish_product/unpublish_product：上架/下架商品 | R4/R6 | 正向 | P0 |
| SV-004 | delete_product：删除商品（退还押金到店铺派生账户） | R4/R6 | 正向 | P1 |
| SV-005 | 超过 MaxProductsPerShop(1000) 被拒绝 | R4 | 负向 | P2 |
| SV-006 | 非授权账户操作被拒绝 | R20 | 权限 | P1 |
| SV-007 | 店铺暂停时无法创建/上架 | R4 | 负向 | P1 |
| SV-008 | 押金随 NEX/USDT 价格动态变化 | R4 | 功能 | P2 |

## 4. Entity Order — 订单管理

> Pallet: `pallet-entity-order` | Extrinsics: 9 (call_index 0-8)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| OD-001 | place_order(NEX)：NEX 支付下单，资金锁入托管 | R5 | 正向 | P0 |
| OD-002 | place_order(shopping_balance)：购物余额抵扣下单 | R5 | 正向 | P0 |
| OD-003 | place_order(Token)：Token 支付订单 | R5 | 正向 | P1 |
| OD-004 | 商品已下架时下单被拒绝 | R5 | 负向 | P0 |
| OD-005 | 余额不足被拒绝 | R5 | 负向 | P0 |
| OD-006 | ship_order→confirm_receipt：卖家发货→买家确认收货→释放资金+触发佣金 | R6→R5 | 流程 | P0 |
| OD-007 | cancel_order：买家取消订单→退款 | R5 | 正向 | P0 |
| OD-008 | 数字商品不可取消/退款 | R5 | 负向 | P1 |
| OD-009 | request_refund→approve_refund：申请退款→卖家同意→退款 | R5→R6 | 流程 | P1 |
| OD-010 | start_service→complete_service→confirm_service：服务类订单流程 | R6→R5 | 流程 | P1 |
| OD-011 | 订单完成触发 update_spent（等级升级 + USDT 独立追踪） | 集成 | 流程 | P0 |
| OD-012 | 订单完成触发 NEX + Token 双路佣金分配 | 集成 | 流程 | P0 |
| OD-013 | 下单时自动注册会员（MemberProvider::ensure_member） | 系统 | 流程 | P0 |
| OD-014 | 非买家/卖家操作被拒绝 | R20 | 权限 | P1 |

## 5. Entity Review — 评价管理

> Pallet: `pallet-entity-review` | Extrinsics: 1 (call_index 0)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| RV-001 | submit_review：买家提交评价（关联已完成订单） | R5 | 正向 | P1 |
| RV-002 | 未完成订单不能评价 | R5 | 负向 | P1 |
| RV-003 | 非订单买家不能评价 | R20 | 权限 | P1 |
| RV-004 | 重复评价被拒绝 | R5 | 负向 | P2 |

## 6. Entity Token — 通证管理

> Pallet: `pallet-entity-token` | Extrinsics: 16 (call_index 0-15)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TK-001 | create_shop_token：创建 7 种 TokenType 代币 | R2 | 正向 | P0 |
| TK-002 | update_token_config：更新配置 | R2 | 正向 | P1 |
| TK-003 | set_max_supply：设置最大供应量 | R2 | 正向 | P1 |
| TK-004 | change_token_type：变更通证类型 | R2 | 正向 | P2 |
| TK-005 | mint_tokens：铸造代币 | R2 | 正向 | P0 |
| TK-006 | 超 max_supply 铸造被拒绝 | R2 | 负向 | P1 |
| TK-007 | transfer_tokens：转让代币 | R8 | 正向 | P0 |
| TK-008 | lock_tokens + unlock_tokens：锁仓+解锁（仅移除已到期条目） | R2/R8 | 正向 | P1 |
| TK-009 | 全部未到期返回 UnlockTimeNotReached（审计 H1 回归） | R8 | 负向 | P1 |
| TK-010 | 无锁仓返回 NoLockedTokens（审计 H1 回归） | R8 | 负向 | P1 |
| TK-011 | configure_dividend + distribute_dividend + claim_dividend：分红全流程 | R2/R8 | 正向 | P1 |
| TK-012 | set_transfer_restriction：设置转账限制(Whitelist/Blacklist/KYC/MembersOnly) | R2 | 正向 | P1 |
| TK-013 | 白名单模式下仅白名单可转账 | R8 | 功能 | P1 |
| TK-014 | 黑名单/KYC/MembersOnly 限制验证 | R8 | 功能 | P1 |
| TK-015 | add_to_whitelist/remove_from_whitelist：添加/移除白名单 | R2 | 正向 | P1 |
| TK-016 | add_to_blacklist/remove_from_blacklist：添加/移除黑名单 | R2 | 正向 | P1 |

## 7. Entity Governance — 实体治理

> Pallet: `pallet-entity-governance` | Extrinsics: 8 (call_index 0-5,9,10)
> 治理模式: None / FullDAO（已简化为 2 种）| 提案类型: 41 种（9 大类）

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GV-001 | configure_governance：配置治理模式 (None/FullDAO) | R2 | 正向 | P1 |
| GV-002 | lock_governance：锁定治理，锁定后仅可升级 FullDAO | R2 | 正向 | P2 |
| GV-003 | create_proposal：创建提案（需 proposal_threshold 代币持有） | R8 | 正向 | P0 |
| GV-004 | 持有不足无法创建提案 | R8 | 负向 | P0 |
| GV-005 | 超过 MaxActiveProposals(10) 被拒绝 | R8 | 负向 | P1 |
| GV-006 | vote：投票（时间加权投票权 = 余额 × 持有时间系数） | R8 | 正向 | P0 |
| GV-007 | 投票期结束后无法投票 | R8 | 负向 | P1 |
| GV-008 | 快照区块防闪贷保护（首次持有时间检查） | R8 | 安全 | P1 |
| GV-009 | finalize_voting：结束投票→计算通过/否决 | R20 | 正向 | P0 |
| GV-010 | 未达法定人数 (quorum_threshold) 否决 | R20 | 功能 | P1 |
| GV-011 | execute_proposal：执行提案（延迟后） | R20 | 正向 | P0 |
| GV-012 | VotingPeriodChange 执行验证 ≥ MinVotingPeriod（审计 H1 回归） | 系统 | 安全 | P1 |
| GV-013 | cancel_proposal：提案者/店主取消 | R8/R2 | 正向 | P1 |
| GV-014 | veto_proposal：管理员否决（需 admin_veto_enabled） | R2/R3 | 正向 | P1 |
| GV-015 | 41 种提案类型执行：商品/店铺/代币/财务/治理/返佣/提现/会员/社区 | 集成 | 流程 | P1 |

详细测试续表见: [NEXUS_TEST_PLAN_PART2.md](./NEXUS_TEST_PLAN_PART2.md)
