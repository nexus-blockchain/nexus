# Nexus 项目全面测试计划

> 基于最新代码的全量接口深度分析，覆盖所有 Pallet 的全部 extrinsic
> 更新日期: 2026-03-08 | 覆盖: 42 链上 Pallet + GroupRobot 链下模块
> 总计 extrinsics: ~700+ | 测试用例: ~920+

---

## 用户角色清单

| # | 角色 | 描述 |
|---|------|------|
| R1 | **Root/Sudo** | 超级管理员（GovernanceOrigin / MarketAdmin / DecisionOrigin） |
| R2 | **Entity Owner** | 实体创建者/所有者 |
| R3 | **Entity Admin** | 实体管理员（≤10 人，位掩码权限） |
| R4 | **Shop Manager** | 店铺管理员 |
| R5 | **Buyer/Consumer** | 买家/消费者 |
| R6 | **Seller/Merchant** | 卖家/服务商 |
| R7 | **Member** | 注册会员 |
| R8 | **Token Holder** | 代币持有者 |
| R9 | **NEX Trader** | P2P 交易者 |
| R10 | **KYC Provider** | KYC 认证机构 |
| R11 | **Arbitrator/Committee** | 仲裁委员（DecisionOrigin） |
| R12 | **Bot Owner** | Bot 所有者 |
| R13 | **Node Operator** | 共识节点运营者 |
| R14 | **Storage Operator** | 存储运营者 |
| R15 | **Community Admin** | 社区管理员 |
| R16 | **Advertiser** | 广告主 |
| R17 | **Subscriber** | Bot 订阅用户 |
| R18 | **Bot Operator** | Bot 运营商 |
| R19 | **OCW** | 链下工作机（Unsigned / None origin） |
| R20 | **普通用户** | 无特殊权限 |
| R21 | **Ad Staker** | 广告质押者 |

---

## 1. Entity Registry — 实体注册管理

> Pallet: `pallet-entity-registry` | Extrinsics: 28 (call_index 0-27)
> 新增: unban_entity, unverify_entity, cancel_close_request, resign_admin, set_primary_shop,
> self_pause_entity, self_resume_entity, force_transfer_ownership, reject_close_request, execute_close_timeout

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| ER-001 | create_entity(0)：付费激活 Active，50 USDT 等值 NEX 押金，自动创建 Primary Shop | R2 | 正向 | P0 |
| ER-002 | 创建不同 EntityType（Merchant/Enterprise/DAO/Community/Project/Fund/Custom） | R2 | 正向 | P1 |
| ER-003 | 创建实体时绑定推荐人（referrer 参数） | R2 | 正向 | P1 |
| ER-004 | 名称为空 / 超长 / 已占用(NameAlreadyTaken) 被拒绝 | R2 | 负向 | P1 |
| ER-005 | 余额不足支付初始资金被拒绝 | R2 | 负向 | P0 |
| ER-006 | 超过 MaxEntitiesPerUser 被拒绝 | R2 | 负向 | P1 |
| ER-007 | 价格不可用时创建被拒绝 | R2 | 负向 | P1 |
| ER-008 | update_entity(1)：Owner/Admin 更新名称/logo/description/metadata/contact | R2/R3 | 正向 | P1 |
| ER-009 | 非 Owner/Admin 无法更新 | R20 | 权限 | P1 |
| ER-010 | request_close_entity(2)：Owner 申请关闭→PendingClose | R2 | 正向 | P1 |
| ER-011 | top_up_fund(3)：任何人充值金库 | R20 | 正向 | P1 |
| ER-012 | ~~approve_entity(4)~~：已移除，付费即激活 | - | - | - |
| ER-013 | ~~非治理 Origin 无法审批~~：已移除 | - | - | - |
| ER-014 | ~~approve_close_entity(5)~~：已移除，关闭统一走超时机制 | - | - | - |
| ER-015 | suspend_entity(6)：治理暂停+原因记录 | R1 | 正向 | P1 |
| ER-016 | resume_entity(7)：治理恢复（需资金充足） | R1 | 正向 | P1 |
| ER-017 | ban_entity(8)：封禁（confiscate_fund=true 没收到国库 / false 保留） | R1 | 正向 | P1 |
| ER-018 | add_admin(9)：添加管理员（权限位掩码），超 10 人被拒绝 | R2 | 正向+负向 | P1 |
| ER-019 | remove_admin(10)：移除管理员 | R2 | 正向 | P1 |
| ER-020 | transfer_ownership(11)：转移所有权（SameOwner 被拒绝） | R2 | 正向+负向 | P1 |
| ER-021 | upgrade_entity_type(12)：升级实体类型 | R1 | 正向 | P2 |
| ER-022 | verify_entity(14) / unverify_entity(19)：验证/取消验证 | R1 | 正向 | P2 |
| ER-023 | reopen_entity(15)：Owner 重开已关闭→直接 Active（付费即激活） | R2 | 正向 | P2 |
| ER-024 | bind_entity_referrer(16)：绑定推荐人（SelfReferral/已绑定拒绝） | R2 | 正向+负向 | P2 |
| ER-025 | update_admin_permissions(17)：更新管理员权限 | R2 | 正向 | P1 |
| ER-026 | **unban_entity(18)：解除封禁→直接 Active（需资金充足）** | R1 | 正向 | P1 |
| ER-027 | **cancel_close_request(20)：取消关闭请求** | R2 | 正向 | P1 |
| ER-028 | **resign_admin(21)：管理员主动辞职** | R3 | 正向 | P1 |
| ER-029 | **set_primary_shop(22)：设置主店铺** | R2 | 正向 | P1 |
| ER-030 | **self_pause_entity(23) / self_resume_entity(24)：Owner 自主暂停/恢复** | R2 | 正向 | P1 |
| ER-031 | 已处于 OwnerPaused 状态重复暂停被拒绝 | R2 | 负向 | P1 |
| ER-032 | **force_transfer_ownership(25)：治理强制转移所有权** | R1 | 正向 | P1 |
| ER-033 | **reject_close_request(26)：治理拒绝关闭请求** | R1 | 正向 | P2 |
| ER-034 | **execute_close_timeout(27)：关闭请求超时自动执行** | R20 | 正向 | P2 |
| ER-035 | Banned 状态不可 reopen（仅 Closed 可重开） | R2 | 负向 | P2 |
| ER-036 | EntityLocked 状态下操作被拒绝 | R2 | 负向 | P1 |

## 2. Entity Shop — 店铺管理

> Pallet: `pallet-entity-shop` | Extrinsics: 33 (call_index 0-32)
> 新增: finalize_close_shop, manager_issue_points, manager_burn_points, redeem_points,
> transfer_shop, force_pause_shop, set_points_ttl, expire_points, force_close_shop,
> set_business_hours, set_shop_policies, set_shop_type, cancel_close_shop,
> set_points_max_supply, resign_manager, ban_shop, unban_shop

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SH-001 | create_shop(0)：关联已激活实体创建店铺 | R2/R3 | 正向 | P0 |
| SH-002 | 实体未激活时创建被拒绝 | R2 | 负向 | P0 |
| SH-003 | 超过 MaxShopsPerEntity 被拒绝 | R2 | 负向 | P1 |
| SH-004 | update_shop(1)：更新名称/logo/描述 | R4 | 正向 | P1 |
| SH-005 | add_manager(2) / remove_manager(3)：添加/移除管理员 | R2 | 正向 | P1 |
| SH-006 | fund_operating(4)：充值运营资金（零金额拒绝） | R2/R4 | 正向+负向 | P1 |
| SH-007 | pause_shop(5) / resume_shop(6)：暂停/恢复 | R2/R4 | 正向 | P1 |
| SH-008 | set_location(7)：设置位置/营业时间 | R4 | 正向 | P1 |
| SH-009 | enable_points(8) / disable_points(10)：启用/禁用积分 | R2 | 正向 | P1 |
| SH-010 | close_shop(9)：关闭店铺 | R2 | 正向 | P1 |
| SH-011 | update_points_config(11)：更新积分配置 | R2 | 正向 | P2 |
| SH-012 | transfer_points(12)：用户间转移积分 | R20 | 正向 | P1 |
| SH-013 | withdraw_operating_fund(13)：提取运营资金（安全检查） | R2 | 正向 | P1 |
| SH-014 | set_customer_service(14)：设置客服账户 | R2 | 正向 | P2 |
| SH-015 | **finalize_close_shop(15)：最终关闭店铺** | R2 | 正向 | P1 |
| SH-016 | **manager_issue_points(16)：管理员发行积分** | R4 | 正向 | P1 |
| SH-017 | **manager_burn_points(17)：管理员销毁积分** | R4 | 正向 | P1 |
| SH-018 | **redeem_points(18)：用户兑换积分** | R20 | 正向 | P1 |
| SH-019 | **transfer_shop(19)：转移店铺到另一实体** | R2 | 正向 | P1 |
| SH-020 | **set_primary_shop(20)：设置主店铺** | R2 | 正向 | P1 |
| SH-021 | **force_pause_shop(21)：Root 强制暂停** | R1 | 正向 | P1 |
| SH-022 | **set_points_ttl(22)：设置积分有效期** | R2 | 正向 | P2 |
| SH-023 | **expire_points(23)：过期用户积分** | R20 | 正向 | P2 |
| SH-024 | **force_close_shop(24)：Root 强制关闭** | R1 | 正向 | P1 |
| SH-025 | **set_business_hours(25)：设置营业时间** | R4 | 正向 | P2 |
| SH-026 | **set_shop_policies(26)：设置店铺政策** | R4 | 正向 | P2 |
| SH-027 | **set_shop_type(27)：变更店铺类型** | R2 | 正向 | P2 |
| SH-028 | **cancel_close_shop(28)：取消关闭** | R2 | 正向 | P1 |
| SH-029 | **set_points_max_supply(29)：设置积分最大供应** | R2 | 正向 | P2 |
| SH-030 | **resign_manager(30)：管理员主动辞职** | R4 | 正向 | P1 |
| SH-031 | **ban_shop(31) / unban_shop(32)：Root 封禁/解封** | R1 | 正向 | P1 |
| SH-032 | 已关闭店铺不可 pause/resume/操作 | R2 | 负向 | P1 |
| SH-033 | 提取超过可用余额被拒绝 | R2 | 安全 | P0 |
| SH-034 | 非授权账户操作被拒绝 | R20 | 权限 | P1 |

## 3. Entity Product — 商品管理

> Pallet: `pallet-entity-product` | Extrinsics: 9 (call_index 0-8)
> 新增: force_unpublish_product, batch_publish_products, batch_unpublish_products, batch_delete_products

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SV-001 | create_product(0)：创建商品（扣 1 USDT 等值押金） | R4/R6 | 正向 | P0 |
| SV-002 | 空 name_cid / images_cid / detail_cid 被拒绝 | R4 | 负向 | P1 |
| SV-003 | update_product(1)：更新商品信息 | R4/R6 | 正向 | P1 |
| SV-004 | publish_product(2) / unpublish_product(3)：上架/下架 | R4/R6 | 正向 | P0 |
| SV-005 | delete_product(4)：删除商品（退还押金） | R4/R6 | 正向 | P1 |
| SV-006 | **force_unpublish_product(5)：Root 强制下架（含 reason）** | R1 | 正向 | P1 |
| SV-007 | **batch_publish_products(6)：批量上架** | R4/R6 | 正向 | P1 |
| SV-008 | **batch_unpublish_products(7)：批量下架** | R4/R6 | 正向 | P1 |
| SV-009 | **batch_delete_products(8)：批量删除** | R4/R6 | 正向 | P1 |
| SV-010 | OnSale 状态下 stock=0 被拒绝 | R4 | 负向 | P1 |
| SV-011 | 店铺暂停时无法创建/上架 | R4 | 负向 | P1 |
| SV-012 | 非授权账户操作被拒绝 | R20 | 权限 | P1 |
| SV-013 | 超过 MaxProductsPerShop 被拒绝 | R4 | 负向 | P2 |

## 4. Entity Order — 订单管理

> Pallet: `pallet-entity-order` | Extrinsics: 18 (call_index 0-17)
> 新增: set_platform_fee_rate, clean_buyer_orders, force_refund, force_complete,
> update_shipping_address, extend_confirm_timeout, update_tracking, seller_cancel_order, clean_shop_orders

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| OD-001 | place_order(0, NEX)：NEX 支付下单，资金锁入托管 | R5 | 正向 | P0 |
| OD-002 | place_order(0, shopping_balance)：购物余额抵扣 | R5 | 正向 | P0 |
| OD-003 | place_order(0, Token)：Token 支付 | R5 | 正向 | P1 |
| OD-004 | 商品已下架/余额不足被拒绝 | R5 | 负向 | P0 |
| OD-005 | cancel_order(1)：买家取消→退款 | R5 | 正向 | P0 |
| OD-006 | ship_order(2)→confirm_receipt(3)：发货→确认→释放+佣金 | R6→R5 | 流程 | P0 |
| OD-007 | request_refund(4)→approve_refund(5)→reject_refund(6)：退款流程 | R5→R6 | 流程 | P1 |
| OD-008 | start_service(7)→complete_service(8)：服务类订单 | R6→R5 | 流程 | P1 |
| OD-009 | **set_platform_fee_rate(9)：设置平台费率（Root）** | R1 | 正向 | P1 |
| OD-010 | **clean_buyer_orders(10)：清理买家订单记录** | R20 | 正向 | P2 |
| OD-011 | **force_refund(11)：Root 强制退款** | R1 | 正向 | P1 |
| OD-012 | **force_complete(12)：Root 强制完成** | R1 | 正向 | P1 |
| OD-013 | **update_shipping_address(13)：更新收货地址（发货前）** | R5 | 正向 | P1 |
| OD-014 | **extend_confirm_timeout(14)：延长确认超时** | R5 | 正向 | P1 |
| OD-015 | **update_tracking(15)：更新物流信息** | R6 | 正向 | P1 |
| OD-016 | **seller_cancel_order(16)：卖家取消订单** | R6 | 正向 | P1 |
| OD-017 | **clean_shop_orders(17)：清理店铺订单记录** | R4 | 正向 | P2 |
| OD-018 | 订单完成触发 NEX + Token 双路佣金 | 系统 | 流程 | P0 |
| OD-019 | 下单时自动注册会员 | 系统 | 流程 | P0 |
| OD-020 | 订单取消触发 cancel_commission + restore_stock | 系统 | 流程 | P0 |
| OD-021 | 非买家/卖家操作被拒绝 | R20 | 权限 | P1 |

## 5. Entity Review — 评价管理

> Pallet: `pallet-entity-review` | Extrinsics: 5 (call_index 0-4)
> 新增: remove_review, reply_to_review, edit_review

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| RV-001 | submit_review(0)：买家评价（关联已完成订单） | R5 | 正向 | P1 |
| RV-002 | set_review_enabled(1)：Entity 级评价开关 | R2 | 正向 | P1 |
| RV-003 | **remove_review(2)：Root 删除评价** | R1 | 正向 | P1 |
| RV-004 | **reply_to_review(3)：卖家回复评价** | R6 | 正向 | P1 |
| RV-005 | **edit_review(4)：买家编辑评价** | R5 | 正向 | P1 |
| RV-006 | 评价已关闭 / 未完成订单 / 非买家评价被拒绝 | R5/R20 | 负向 | P1 |
| RV-007 | 重复评价被拒绝 | R5 | 负向 | P2 |

## 6. Entity Token — 通证管理

> Pallet: `pallet-entity-token` | Extrinsics: 25 (call_index 0-24)
> 新增: force_disable_token, force_freeze/unfreeze_transfers, force_burn, set_global_token_pause,
> burn_tokens, update_token_metadata, force_transfer, force_enable_token

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TK-001 | create_shop_token(0)：创建 7 种 TokenType 代币 | R2 | 正向 | P0 |
| TK-002 | update_token_config(1)：更新配置 | R2 | 正向 | P1 |
| TK-003 | mint_tokens(2)：铸造代币 | R2 | 正向 | P0 |
| TK-004 | transfer_tokens(3)：转让代币 | R8 | 正向 | P0 |
| TK-005 | configure_dividend(4) + distribute_dividend(5) + claim_dividend(6)：分红全流程 | R2/R8 | 正向 | P1 |
| TK-006 | lock_tokens(7) + unlock_tokens(8)：锁仓+解锁（仅已到期） | R2/R8 | 正向 | P1 |
| TK-007 | change_token_type(9)：变更通证类型 | R2 | 正向 | P2 |
| TK-008 | set_max_supply(10)：设置最大供应 | R2 | 正向 | P1 |
| TK-009 | set_transfer_restriction(11)：Whitelist/Blacklist/KYC/MembersOnly | R2 | 正向 | P1 |
| TK-010 | add_to_whitelist(12) / remove_from_whitelist(13) | R2 | 正向 | P1 |
| TK-011 | add_to_blacklist(14) / remove_from_blacklist(15) | R2 | 正向 | P1 |
| TK-012 | **force_disable_token(16) / force_enable_token(24)：Root 禁用/启用** | R1 | 正向 | P1 |
| TK-013 | **force_freeze_transfers(17) / force_unfreeze_transfers(18)：Root 冻结/解冻** | R1 | 正向 | P1 |
| TK-014 | **force_burn(19)：Root 强制销毁** | R1 | 正向 | P1 |
| TK-015 | **set_global_token_pause(20)：全局代币暂停** | R1 | 正向 | P1 |
| TK-016 | **burn_tokens(21)：用户主动销毁** | R8 | 正向 | P1 |
| TK-017 | **update_token_metadata(22)：更新代币元数据** | R2 | 正向 | P2 |
| TK-018 | **force_transfer(23)：Root 强制转账** | R1 | 正向 | P1 |
| TK-019 | 超 max_supply 铸造被拒绝 | R2 | 负向 | P1 |
| TK-020 | 全局暂停时所有代币操作被拒绝 | R8 | 安全 | P0 |
| TK-021 | 冻结时转账被拒绝 | R8 | 安全 | P0 |

## 7. Entity Governance — 实体治理

> Pallet: `pallet-entity-governance` | Extrinsics: 9 (call_index 0-5, delegate_vote, undelegate_vote, admin_veto)
> 新增: delegate_vote, undelegate_vote, admin_veto

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GV-001 | configure_governance(5)：配置模式 (None/FullDAO) | R2 | 正向 | P1 |
| GV-002 | create_proposal(0)：创建提案（需 threshold 持有） | R8 | 正向 | P0 |
| GV-003 | 持有不足 / 超过 MaxActiveProposals 被拒绝 | R8 | 负向 | P0 |
| GV-004 | vote(1)：投票（时间加权） | R8 | 正向 | P0 |
| GV-005 | **delegate_vote / undelegate_vote：委托/取消委托投票** | R8 | 正向 | P1 |
| GV-006 | execute_proposal(3)：执行提案 | R20 | 正向 | P0 |
| GV-007 | cancel_proposal(4)：取消提案 | R8/R2 | 正向 | P1 |
| GV-008 | **admin_veto：管理员否决提案** | R2/R3 | 正向 | P1 |
| GV-009 | lock_governance：锁定治理（不可撤销） | R2 | 正向 | P2 |
| GV-010 | 投票期结束后无法投票 | R8 | 负向 | P1 |
| GV-011 | 未达法定人数否决 | R20 | 功能 | P1 |
| GV-012 | 快照区块防闪贷保护 | R8 | 安全 | P1 |
| GV-013 | 41 种提案类型执行验证 | 集成 | 流程 | P1 |

详细测试续表见: [NEXUS_TEST_PLAN_PART2.md](./NEXUS_TEST_PLAN_PART2.md)
