# Nexus 项目全面测试计划

> 基于所有用户角色的功能性、流程性测试清单
> 更新日期: 2026-03-03 | 覆盖: 37 链上 Pallet + GroupRobot 链下模块
> 基于最新代码 + 全量审计修复后重新生成，包含回归测试点

---

## 用户角色清单

| # | 角色 | 描述 |
|---|------|------|
| R1 | **Root/Sudo** | 超级管理员（GovernanceOrigin） |
| R2 | **Entity Owner** | 实体创建者/所有者 |
| R3 | **Entity Admin** | 实体管理员（≤10人，位掩码权限） |
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

## E2E 测试基础设施

| 组件 | 路径 | 说明 |
|------|------|------|
| **Config** | `scripts/e2e/core/config.ts` | 环境配置（ws://127.0.0.1:9944, NEX=10^12, USDT=10^6） |
| **Test Runner** | `scripts/e2e/core/test-runner.ts` | 测试执行引擎 |
| **Assertions** | `scripts/e2e/core/assertions.ts` | 链上状态断言库 |
| **Chain State** | `scripts/e2e/core/chain-state.ts` | 链上状态查询 |
| **Coverage** | `scripts/e2e/core/coverage-tracker.ts` | 覆盖率追踪 |
| **Nexus Agent** | `scripts/e2e/nexus-test-agent.ts` | 智能测试代理 |

### 已有 E2E Flow 文件

| 目录 | 文件 | 覆盖模块 |
|------|------|----------|
| `flows/entity/` | `entity-shop.ts`, `order-lifecycle.ts`, `commission.ts`, `member-referral.ts`, `token-governance.ts`, `token-sale.ts`, `kyc.ts` | Entity 全栈 |
| `flows/trading/` | `p2p-sell.ts`, `p2p-buy.ts`, `maker-lifecycle.ts` | NEX Market |
| `flows/grouprobot/` | `bot-lifecycle.ts`, `node-consensus.ts`, `ad-campaign.ts` | GroupRobot |
| `flows/dispute/` | `dispute-resolution.ts` | 争议解决 |
| `flows/storage/` | `storage-service.ts` | 存储服务 |

---

## 1. Entity Registry — 实体注册管理

> Pallet: `pallet-entity-registry` | Extrinsics: 17 (call_index 0-12, 14-17; 13已移除)
> 审计改名: `get_cos_usdt_price()` → `get_nex_usdt_price()`

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| ER-001 | create_entity(Merchant)：付费即激活 Active，50 USDT 等值 NEX 押金，自动创建 Primary Shop | R2 | 正向 | P0 |
| ER-002 | 创建不同 EntityType（Enterprise/DAO/Community/Project/Fund/Custom） | R2 | 正向 | P1 |
| ER-003 | 创建实体时绑定推荐人（referrer 参数） | R2 | 正向 | P1 |
| ER-004 | 名称为空/超过 BoundedVec 限长被拒绝 | R2 | 负向 | P1 |
| ER-005 | 余额不足支付初始资金被拒绝 | R2 | 负向 | P0 |
| ER-006 | 超过 MaxEntitiesPerUser(3) 被拒绝 | R2 | 负向 | P1 |
| ER-007 | 价格不可用时(get_nex_usdt_price=0)创建被拒绝 | R2 | 负向 | P1 |
| ER-008 | update_entity(1)：Owner/Admin 更新实体名称和 logo | R2/R3 | 正向 | P1 |
| ER-009 | 非 Owner/Admin 无法更新 | R20 | 权限 | P1 |
| ER-010 | request_close_entity(2)：Owner 申请关闭→PendingClose，active_entities 递减 | R2 | 正向 | P1 |
| ER-011 | top_up_fund(3)：任何人充值金库 | R20 | 正向 | P1 |
| ER-012 | approve_entity(4)：治理审批 reopen 后的 Pending→Active，恢复关联 Shop | R1 | 正向 | P0 |
| ER-013 | 非治理 Origin 无法审批 | R20 | 权限 | P0 |
| ER-014 | approve_close_entity(5)：治理审批关闭→退还全部余额到 Owner | R1 | 正向 | P1 |
| ER-015 | suspend_entity(6)：治理暂停实体 | R1 | 正向 | P1 |
| ER-016 | resume_entity(7)：治理恢复实体（需资金充足） | R1 | 正向 | P1 |
| ER-017 | 恢复时资金不足被拒绝 | R1 | 负向 | P1 |
| ER-018 | ban_entity(8)：治理封禁实体（confiscate_fund=true 没收资金到国库） | R1 | 正向 | P1 |
| ER-019 | ban_entity：confiscate_fund=false 保留资金 | R1 | 正向 | P2 |
| ER-020 | add_admin(9)：添加管理员（指定权限位掩码），超过 10 人被拒绝 | R2 | 正向+负向 | P1 |
| ER-021 | remove_admin(10)：移除管理员 | R2 | 正向 | P1 |
| ER-022 | update_admin_permissions(17)：更新管理员权限位掩码 | R2 | 正向 | P1 |
| ER-023 | transfer_ownership(11)：转移所有权 | R2 | 正向 | P1 |
| ER-024 | upgrade_entity_type(12)：升级实体类型（治理） | R1 | 正向 | P2 |
| ER-025 | verify_entity(14)：验证实体（治理） | R1 | 正向 | P2 |
| ER-026 | reopen_entity(15)：Owner 重开已关闭实体→Pending→需 approve_entity | R2 | 正向 | P2 |
| ER-027 | Banned 状态实体不可 reopen（仅 Closed 可重开） | R2 | 负向 | P2 |
| ER-028 | bind_entity_referrer(16)：绑定推荐人 | R2 | 正向 | P2 |
| ER-029 | 不能推荐自己 / 已绑定不可重复 / 推荐人需有 Active Entity | R2 | 负向 | P2 |
| ER-030 | 资金健康状态检测（treasury balance vs MinOperatingBalance） | R20 | 功能 | P2 |

## 2. Entity Shop — 店铺管理

> Pallet: `pallet-entity-shop` | Extrinsics: 14 (call_index 0-13)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SH-001 | create_shop(0)：关联已激活实体创建店铺 | R2/R3 | 正向 | P0 |
| SH-002 | 实体未激活时创建被拒绝 | R2 | 负向 | P0 |
| SH-003 | 超过 MaxShopsPerEntity(16) 被拒绝 | R2 | 负向 | P1 |
| SH-004 | update_shop(1)：更新名称/logo/描述 | R4 | 正向 | P1 |
| SH-005 | 空 CID (logo_cid/description_cid) 被拒绝 (EmptyCid) | R4 | 负向 | P1 |
| SH-006 | set_location(7)：设置位置/营业时间，空 address_cid 被拒绝 | R4 | 正向+负向 | P1 |
| SH-007 | 非授权账户更新被拒绝（can_manage_shop 校验） | R20 | 权限 | P1 |
| SH-008 | add_manager(2)/remove_manager(3)：添加/移除店铺管理员 | R2 | 正向 | P1 |
| SH-009 | fund_operating(4)：充值运营资金，零金额被拒绝 (ZeroFundAmount) | R2/R4 | 正向+负向 | P1 |
| SH-010 | withdraw_operating_fund(13)：提取运营资金 | R2 | 正向 | P1 |
| SH-011 | 提取超过可用余额(扣除 pending+shopping 佣金保护)被拒绝 | R2 | 安全 | P0 |
| SH-012 | 活跃 Shop 提取后余额不低于 MinOperatingBalance | R2 | 安全 | P0 |
| SH-013 | 已关闭 Shop 无最低余额限制，可全额提取 | R2 | 正向 | P1 |
| SH-014 | pause_shop(5)/resume_shop(6)：暂停/恢复店铺 | R2/R4 | 正向 | P1 |
| SH-015 | 已关闭 Shop 不可 pause/resume (状态校验) | R2 | 负向 | P1 |
| SH-016 | close_shop(9)：关闭店铺 | R2 | 正向 | P1 |
| SH-017 | enable_points(8)/disable_points(10)：启用/禁用积分系统 | R2 | 正向 | P1 |
| SH-018 | update_points_config(11)：更新积分配置（已关闭店铺被拒绝） | R2 | 正向+负向 | P2 |
| SH-019 | transfer_points(12)：用户间转移积分（已关闭店铺被拒绝） | R20 | 正向+负向 | P1 |
| SH-020 | 积分余额不足转移被拒绝 | R20 | 负向 | P1 |

## 3. Entity Service — 商品/服务管理

> Pallet: `pallet-entity-product` | Extrinsics: 5 (call_index 0-4)
> 审计修复: H1(OffShelf 库存恢复), H2(SoldCountUpdated 事件), M1(零库存防护), M2(CID 非空), M3(stale price)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| SV-001 | create_product(0)：创建商品（从店铺派生账户扣取 1 USDT 等值 NEX 押金） | R4/R6 | 正向 | P0 |
| SV-002 | 空 name_cid / images_cid / detail_cid 被拒绝（审计 M2 回归） | R4 | 负向 | P1 |
| SV-003 | update_product(1)：更新商品信息 | R4/R6 | 正向 | P1 |
| SV-004 | OnSale 状态下 stock=0 被拒绝 CannotClearStockWhileOnSale（审计 M1 回归） | R4 | 负向 | P1 |
| SV-005 | Draft 状态下 stock=0 允许（无限库存语义） | R4 | 正向 | P2 |
| SV-006 | publish_product(2)/unpublish_product(3)：上架/下架商品 | R4/R6 | 正向 | P0 |
| SV-007 | delete_product(4)：删除商品（退还押金到店铺派生账户） | R4/R6 | 正向 | P1 |
| SV-008 | 超过 MaxProductsPerShop(1000) 被拒绝 | R4 | 负向 | P2 |
| SV-009 | 非授权账户操作被拒绝 | R20 | 权限 | P1 |
| SV-010 | 店铺暂停时无法创建/上架 | R4 | 负向 | P1 |
| SV-011 | 押金随 NEX/USDT 价格动态变化（stale 时返回 min_deposit，审计 M3） | R4 | 功能 | P2 |
| SV-012 | restore_stock 对 OffShelf+stock=0 正确恢复库存（审计 H1 回归） | 系统 | 功能 | P0 |
| SV-013 | add_sold_count 发出 SoldCountUpdated 事件（审计 H2 回归） | 系统 | 功能 | P1 |

## 4. Entity Order — 订单管理

> Pallet: `pallet-entity-order` | Extrinsics: 9 (call_index 0-8)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| OD-001 | place_order(0, NEX)：NEX 支付下单，资金锁入托管 | R5 | 正向 | P0 |
| OD-002 | place_order(0, shopping_balance)：购物余额抵扣下单（NEX 从 Entity→买家→托管） | R5 | 正向 | P0 |
| OD-003 | place_order(0, Token)：Token 支付订单 | R5 | 正向 | P1 |
| OD-004 | 商品已下架时下单被拒绝 | R5 | 负向 | P0 |
| OD-005 | 余额不足被拒绝 | R5 | 负向 | P0 |
| OD-006 | ship_order(2)→confirm_receipt(3)：卖家发货→买家确认收货→释放资金+触发佣金 | R6→R5 | 流程 | P0 |
| OD-007 | cancel_order(1)：买家取消订单→退款（数字商品不可取消） | R5 | 正向+负向 | P0 |
| OD-008 | request_refund(4)→approve_refund(5)：申请退款→卖家同意→退款 | R5→R6 | 流程 | P1 |
| OD-009 | start_service(6)→complete_service(7)→confirm_service(8)：服务类订单流程 | R6→R5 | 流程 | P1 |
| OD-010 | 订单完成触发 update_spent（等级升级 + USDT 消费累计，审计 P4） | 集成 | 流程 | P0 |
| OD-011 | 订单完成触发 NEX + Token 双路佣金分配（process_commission + process_token_commission） | 集成 | 流程 | P0 |
| OD-012 | 下单时自动注册会员（MemberProvider::ensure_member） | 系统 | 流程 | P0 |
| OD-013 | 订单取消触发 cancel_commission + do_cancel_token_commission | 系统 | 流程 | P0 |
| OD-014 | 订单取消触发 restore_stock（含 OffShelf 产品库存恢复，审计 H1） | 系统 | 流程 | P1 |
| OD-015 | 非买家/卖家操作被拒绝 | R20 | 权限 | P1 |

## 5. Entity Review — 评价管理

> Pallet: `pallet-entity-review` | Extrinsics: 2 (call_index 0-1)
> 审计: R6(ShopReviewCount best-effort), R7(entity_id 缓存, entity rating 失败 best-effort)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| RV-001 | submit_review(0)：买家提交评价（关联已完成订单），更新 Shop + Entity rating | R5 | 正向 | P1 |
| RV-002 | set_review_enabled(1)：Entity 级评价开关 | R2 | 正向 | P1 |
| RV-003 | Entity 评价已关闭时提交被拒绝 (EntityReviewDisabled) | R5 | 负向 | P1 |
| RV-004 | 未完成订单不能评价 | R5 | 负向 | P1 |
| RV-005 | 非订单买家不能评价 | R20 | 权限 | P1 |
| RV-006 | 重复评价被拒绝 | R5 | 负向 | P2 |
| RV-007 | Shop/Entity rating 更新失败不回滚评价存储（best-effort，审计 R6/R7 回归） | 系统 | 功能 | P2 |

## 6. Entity Token — 通证管理

> Pallet: `pallet-entity-token` | Extrinsics: 16 (call_index 0-15)
> 审计: H1(unlock_tokens NoLockedTokens/UnlockTimeNotReached), M1(default_transfer_restriction 返回枚举)

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| TK-001 | create_shop_token(0)：创建 7 种 TokenType 代币 | R2 | 正向 | P0 |
| TK-002 | update_token_config(1)：更新配置 | R2 | 正向 | P1 |
| TK-003 | set_max_supply(10)：设置最大供应量 | R2 | 正向 | P1 |
| TK-004 | change_token_type(9)：变更通证类型 | R2 | 正向 | P2 |
| TK-005 | mint_tokens(2)：铸造代币 | R2 | 正向 | P0 |
| TK-006 | 超 max_supply 铸造被拒绝 | R2 | 负向 | P1 |
| TK-007 | transfer_tokens(3)：转让代币 | R8 | 正向 | P0 |
| TK-008 | lock_tokens(7) + unlock_tokens(8)：锁仓+解锁（仅移除已到期条目） | R2/R8 | 正向 | P1 |
| TK-009 | 全部未到期返回 UnlockTimeNotReached（审计 H1 回归） | R8 | 负向 | P1 |
| TK-010 | 无锁仓返回 NoLockedTokens（审计 H1 回归） | R8 | 负向 | P1 |
| TK-011 | 部分到期部分未到期：仅解锁到期条目（审计 H1 回归） | R8 | 正向 | P1 |
| TK-012 | configure_dividend(4) + distribute_dividend(5) + claim_dividend(6)：分红全流程 | R2/R8 | 正向 | P1 |
| TK-013 | set_transfer_restriction(11)：设置转账限制 (Whitelist/Blacklist/KYC/MembersOnly) | R2 | 正向 | P1 |
| TK-014 | 白名单模式下仅白名单可转账 | R8 | 功能 | P1 |
| TK-015 | 黑名单/KYC/MembersOnly 限制验证 | R8 | 功能 | P1 |
| TK-016 | add_to_whitelist(12)/remove_from_whitelist(13)：批量添加/移除白名单 | R2 | 正向 | P1 |
| TK-017 | add_to_blacklist(14)/remove_from_blacklist(15)：批量添加/移除黑名单 | R2 | 正向 | P1 |

## 7. Entity Governance — 实体治理

> Pallet: `pallet-entity-governance` | Extrinsics: 9 (call_index 0-5, 9-11)
> 治理模式: None / FullDAO（已简化为 2 种）| 提案类型: 41 种（9 大类）
> 审计新增: cleanup_proposal(11) 清理终态提案

| # | 测试用例 | 角色 | 类型 | 优先级 |
|---|---------|------|------|--------|
| GV-001 | configure_governance(5)：配置治理模式 (None/FullDAO) | R2 | 正向 | P1 |
| GV-002 | lock_governance(10)：锁定治理（不可撤销，锁定后仅可通过提案修改） | R2 | 正向 | P2 |
| GV-003 | create_proposal(0)：创建提案（需 proposal_threshold 代币持有） | R8 | 正向 | P0 |
| GV-004 | 持有不足无法创建提案 | R8 | 负向 | P0 |
| GV-005 | 超过 MaxActiveProposals(10) 被拒绝 | R8 | 负向 | P1 |
| GV-006 | vote(1)：投票（时间加权投票权 = 余额 × 持有时间系数） | R8 | 正向 | P0 |
| GV-007 | 投票期结束后无法投票 | R8 | 负向 | P1 |
| GV-008 | 快照区块防闪贷保护（首次持有时间检查） | R8 | 安全 | P1 |
| GV-009 | finalize_voting(2)：结束投票→计算通过/否决 | R20 | 正向 | P0 |
| GV-010 | 未达法定人数 (quorum_threshold) 否决 | R20 | 功能 | P1 |
| GV-011 | execute_proposal(3)：执行提案（延迟后） | R20 | 正向 | P0 |
| GV-012 | VotingPeriodChange 执行验证 ≥ MinVotingPeriod（审计 H1 回归） | 系统 | 安全 | P1 |
| GV-013 | cancel_proposal(4)：提案者/店主取消 | R8/R2 | 正向 | P1 |
| GV-014 | veto_proposal(9)：管理员否决（需 admin_veto_enabled） | R2/R3 | 正向 | P1 |
| GV-015 | cleanup_proposal(11)：清理终态提案（Executed/Failed/Cancelled/Expired） | R20 | 正向 | P2 |
| GV-016 | 清理非终态提案被拒绝 | R20 | 负向 | P2 |
| GV-017 | 41 种提案类型执行：商品/店铺/代币/财务/治理/返佣/提现/会员/社区 | 集成 | 流程 | P1 |

详细测试续表见: [NEXUS_TEST_PLAN_PART2.md](./NEXUS_TEST_PLAN_PART2.md)
