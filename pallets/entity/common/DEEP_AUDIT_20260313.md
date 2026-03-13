# pallet-entity-common 深度审计分析（2026-03-13）

我做完了对 `pallet-entity-common` 及其主要消费方（`registry/shop/product/order/member/token/governance/kyc/disclosure/market/tokensale/loyalty/review`）的穿透分析。  
**结论一句话：`common` 现在最大的问题不是“类型不够”，而是“接口定义已经超前于真实落地”，导致不少角色业务流出现了：权限失真、状态失真、治理 no-op、以及 fail-open 风险。**

## 一、按角色看，核心问题是什么

### 1) Entity Owner / Entity Admin
- **权限模型失真**：`PRODUCT_MANAGE / MARKET_MANAGE / GOVERNANCE_MANAGE / ORDER_MANAGE` 已定义，但业务里并没有真正落地成独立岗位权限。  
  例如：
  - `PRODUCT_MANAGE` 定义在 `entity/common/src/types/mod.rs:858-867`
  - 但 product 仍然检查 `SHOP_MANAGE`：`entity/product/src/lib.rs:1261-1264`
  - market 仍然是 owner-only：`entity/market/src/lib.rs:1816-1843`
- **所有权转移流程不一致**：`common` 里已经定义了“两步式转移”接口 `initiate/accept/cancel/pending`（`entity/common/src/traits/core.rs:112-132`），但 registry 仍是**一步式立即转移**（`entity/registry/src/lib.rs:824-833`）。  
  这对 owner 是高风险：填错地址就是实质转移。

### 2) Shop / Product Manager
- **商品可见性治理是空操作**：  
  `governance` 会调用 `ProductProvider::governance_set_visibility`（`entity/governance/src/lib.rs:3155-3162`），但 common 默认实现直接 `Ok(())`（`entity/common/src/traits/core.rs:480-483`），而 product pallet 实际实现里没 override 这个方法（实现只到 `update_price/delist/set_inventory`，见 `entity/product/src/lib.rs:1598-1660`）。  
  => 提案通过，链上不生效。
- **ShopType 枚举与治理执行不一致**：  
  common 有 7 种 `ShopType`（`entity/common/src/types/mod.rs:244-267`），但 governance 只接受 `<=3`，并把 `3` 映射成 `Virtual`（`entity/governance/src/lib.rs:2580-2583`, `3145-3149`）。  
  => owner 能设的类型，治理未必能设。

### 3) Buyer / Member / Referrer / Payer
- **订单状态机失真**：
  - `order_paid_at()` 实际返回的是 `created_at`（`entity/order/src/lib.rs:1732-1734`）  
    => 买家/卖家/前端拿到的是假支付时间。
  - `force_partial_refund` 事件发的是“部分退款”，但状态却写成 `Refunded`，不是 `PartiallyRefunded`（`entity/order/src/dispute.rs:205-213`）  
    => buyer/seller 的订单视图、统计、后续规则都会错。
- **退款/争议状态过粗**：  
  `request_refund`、`reject_refund`、`withdraw_dispute` 都复用 `OrderStatus::Disputed`（`entity/order/src/dispute.rs`），缺少 “退款申请中 / 卖家已拒绝 / 仲裁中” 这样的明确跨模块状态。  
  => 从买家/卖家视角，common 的状态模型不够用。
- **会员停用语义漏掉了**：  
  common 里 `is_member_active()` 默认仅判断“没被 ban”就算 active（`entity/common/src/traits/member.rs:252-255`）；  
  member pallet实现了 `activate_member/deactivate_member`（`entity/member/src/lib.rs:2197-2245`），但 **没有 override `is_member_active()`**，只 override 了 `is_activated()`（`entity/member/src/lib.rs:2361-2364`）。  
  `pool-reward` 这类模块只看 `is_member_active`（`entity/commission/pool-reward/src/lib.rs:1208-1212`）。  
  => **被手动停用的会员，仍可能被当作活跃会员继续领奖励。**

### 4) Token Holder / Investor
- **治理对 token 的三个关键提案当前是 no-op 风险**：
  - `TokenMaxSupplyChange`
  - `TokenTypeChange`
  - `TransferRestrictionChange`
  
  governance 会执行：
  - `entity/governance/src/lib.rs:3107-3121`
  
  但 common 默认方法本身就是 `Ok(())`：
  - `entity/common/src/traits/asset.rs:119-128`
  
  而 token pallet 的 `impl EntityTokenProvider` 并没有实现这几个方法（实现块从 `entity/token/src/lib.rs:2200` 开始，实际只实现了 `available_balance/governance_burn` 等）。  
  => **提案可能“执行成功”，但状态根本没变。**
- **Hybrid 代币默认策略自相矛盾**：  
  `Hybrid` 的 `required_kyc_level()` 是 `(2,2)`，但 `default_transfer_restriction()` 却是 `None`（`entity/common/src/types/mod.rs:491-520`）。  
  token 变更类型时会把这套默认直接写回配置（`entity/token/src/lib.rs:1167-1174`）。  
  => 结果是 **看起来要 KYC，实际上因为 restriction=None 根本不拦截**。
- **Investor 查询接口落地不完整**：  
  common 已定义 `DividendProvider / VestingProvider / MarketProvider / ReviewProvider`，但我在当前 `pallets/entity` 目录里**没找到生产 impl**，只看到 `TokenSaleProvider` 有真实实现（`entity/tokensale/src/lib.rs:2518`）。  
  => 投资者/前端/治理很多“标准查询口”只是接口，不是能力。

### 5) Governance / Compliance / Root
- **治理参数空间落后于 common 枚举**
  - `TokenType` common 有 7 个变体（`entity/common/src/types/mod.rs:453-470`），但 governance 只允许 `<=3`（`entity/governance/src/lib.rs:2539-2540`），执行时也只映射到 4 个类型（`3112-3116`）。
  - `ProductVisibility::LevelGated(u8)` 理论支持任意 level，但 governance 校验 `visibility <= 2`（`entity/governance/src/lib.rs:2587-2589`）。  
  => 治理能力已经落后于 common 的公共语义。
- **关闭实体前的“依赖检查”可能是空安全**：  
  registry 关闭前会检查 `MarketProvider` / `DisputeQueryProvider`（`entity/registry/src/helpers.rs:366-385`），  
  但在当前 `pallets/entity` 目录里我只找到它们的 `Null/mock`，没找到 market/dispute 的生产 impl。  
  => **如果 runtime 也是这么接线，entity 可能在仍有活跃市场/争议时被关闭。**

## 二、我认为“必须新增”的能力

1. **把关键写接口改成 fail-closed**
   - 现在很多 Null/默认写接口直接 `Ok(())`：
     - `NullEntityTokenProvider`：`entity/common/src/traits/asset.rs:141-169`
     - `NullMemberProvider`：`entity/common/src/traits/member.rs:431-438`
     - 6 个 GovernancePort 的 `()` 实现：`entity/common/src/traits/governance_ports.rs:31-39`
     - README 还明确写了“写入类方法返回 `Ok(())`”：`entity/common/README.md:562`
   - 对金融/治理系统，这是**结构性漏洞**。应改成：
     - 关键资产/治理写操作：默认 `Err(not wired)`
     - 只有显式声明 optional 的功能才允许 no-op

2. **补齐真正的生产 Provider 实现**
   - 至少要补：
     - `MarketProvider`
     - `DisputeQueryProvider`
     - `ReviewProvider`
     - `DividendProvider`
     - `VestingProvider`
     - `EmergencyProvider`

3. **把 Entity 所有权转移统一成两步式**
   - `common` 已有接口，但 registry 未实现。
   - 这是 owner 角色的刚需，不只是“优化”。

4. **重做订单状态公共模型**
   - 至少拆出：
     - `RefundRequested`
     - `RefundRejected`
     - `Arbitrating/Mediating`（若跨 dispute）
     - `PartiallyRefunded`
   - 否则 buyer/seller/frontend/indexer 都会混淆。

5. **做“角色模板”而不是只有位掩码**
   - common 现在是技术权限，不是业务岗位。
   - 建议内置模板：
     - `EntityOpsAdmin`
     - `ProductAdmin`
     - `MarketAdmin`
     - `ComplianceAdmin`
     - `CommissionAdmin`

## 三、冗余/死代码/语义漂移

1. **`MemberStatus` 基本是死枚举**
   - 检索结果显示几乎只在 common/tests 出现，member pallet 自己并不以它做真实状态源。
2. **`OrderStatus::Processing / AwaitingConfirmation` 已进入 common，但 order pallet 没实际使用**
3. **`ProductCategory::Subscription / Bundle` 已进入 common，但 product pallet 明确拒绝创建/更新**
   - 定义：`entity/common/src/types/mod.rs:635-638`
   - 拒绝：`entity/product/src/lib.rs:440-442`, `672-673`
4. **`AdminPermission::ALL` 是 API 陷阱**
   - `ALL = 0xFFFF_FFFF`（`entity/common/src/types/mod.rs:868-869`）
   - 但 `is_valid()` 只允许 `ALL_DEFINED`（`885-887`）
   - 名字叫 ALL，实际上是非法值。
5. **`EntityType::Custom(u8)` 虽已标为 deprecated，但仍公开暴露**
   - 注释自己就承认它像“策略后门”：`entity/common/src/types/mod.rs:31-36`

## 四、明确代码 BUG / 漏洞清单

### P0
1. **治理提案执行成功但 token 参数不落地**
2. **治理修改商品可见性是 no-op**
3. **Null/默认写接口 fail-open**
4. **关闭实体的活跃市场/争议检查可能失效**
5. **停用会员仍可能领取奖励**

### P1
6. **`order_paid_at()` 返回错字段**
7. **部分退款状态写错成 `Refunded`**
8. **治理只支持 4/7 个 TokenType**
9. **治理的 LevelGated 可见性被硬编码限制到 `<=2`**
10. **治理 `AddCustomLevel(level_id, …)` 中的 `level_id` 被 member pallet 忽略**
11. **Hybrid token 默认会把 KYC 要求“写出来但不执行”**

## 五、建议的修复优先级

### 第一批，必须先修
- `governance_set_*` no-op 问题
- `governance_set_visibility` no-op 问题
- `is_member_active` 语义修正
- `force_partial_refund` 状态修正
- Null/() 写接口 fail-open 改为 fail-closed

### 第二批
- 补真实 Provider impl
- 补两步式 ownership transfer
- 统一 TokenType / ShopType / ProductVisibility 的治理编码
- 清理死枚举/死权限位

### 第三批
- 做角色模板
- 做更完整的订单/争议公共状态机
- 给 common 增加“必接线接口”和“可选接口”的明确分层

如果你愿意，我下一步可以直接给你输出一份**按 P0/P1/P2 排序、带修改点文件清单的修复方案**。
