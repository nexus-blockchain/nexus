# Entity 模块主网缺失功能分析

> 分析时间: 2026-03  
> 范围: pallets/entity/ 下全部 22 个子模块  
> 方法: 逐模块源码审查，识别主网部署必须但尚未实现的功能

## 总览

经过对 Entity 模块全部子模块的深度分析，识别出以下主网关键缺失项：

| 编号 | 严重度 | 模块 | 缺失功能 | 说明 |
|------|--------|------|----------|------|
| C1 | Critical | 8+ pallets | **WeightInfo/Benchmarking 缺失** | registry, shop, order, governance, market, disclosure, member, tokensale, kyc 等使用硬编码 Weight，无基准测试 |
| C2 | Critical | 全部 | **StorageVersion + on_runtime_upgrade 缺失** | 无存储版本管理，主网升级时无法安全迁移存储 |
| C3 | Critical | governance | **提案过期自动清理缺失** | Voting 状态提案过期后无自动转换为 Expired 的 hook，存储永久残留 |
| ~~H1~~ | ~~High~~ | ~~order~~ | ~~平台费去向未实现~~ | ✅ 已确认正确: line 1246 通过 Escrow 转给 PlatformAccount |
| H2 | High | market | **OCW 验证器无签名验证** | submit_ocw_result 缺少 ValidateUnsigned 实现，任何人可伪造 OCW 结果 |
| H3 | High | tokensale | **on_initialize 缺少 weight 返回** | 自动结束发售的 hook 未正确计算消耗的 weight |
| H4 | High | governance | **do_execute_proposal 多种类型未实现** | TokenBurn, AirdropDistribution, Dividend 等提案类型执行为空操作 |
| M1 | Medium | registry | **Entity 数据无链上搜索/分页查询** | 无 Runtime API 供前端批量查询 Entity 列表 |
| M2 | Medium | shop | **Shop 关闭宽限期无自动完成** | ShopClosingAt 记录存在但无 on_idle/on_initialize 自动检查完成 |
| M3 | Medium | product | **商品无批量操作** | 无批量上下架、批量更新库存等运营必需功能 |
| M4 | Medium | market | **24h 统计数据无自动刷新** | DailyTradeStats 的 window_start 需手动触发刷新 |
| M5 | Medium | order | **订单无批量查询 Runtime API** | 买家/卖家订单列表无分页 API |
| M6 | Medium | tokensale | **Vesting 解锁无自动 hook** | 用户必须手动调用 unlock_tokens，无自动批量解锁 |

---

## 详细分析

### C1: WeightInfo/Benchmarking 缺失 [Critical]

**影响模块**: pallet-entity-registry, pallet-entity-shop, pallet-entity-order, pallet-entity-governance, pallet-entity-market, pallet-entity-disclosure, pallet-entity-member, pallet-entity-tokensale, pallet-entity-kyc, pallet-entity-commission-core

**现状**: 
- 仅 4 个模块有 `weights.rs`: token, review, multi-level, pool-reward
- 其余 8+ 个核心模块全部使用 `Weight::from_parts(硬编码, 硬编码)` 作为 weight 注解
- 无 benchmarking.rs 文件

**主网风险**: 
- 硬编码 weight 无法反映真实执行成本
- 可能导致区块超重（拒绝交易）或低估（DoS 攻击向量）
- 验证者无法准确计价

**修复方案**: 为每个缺失模块创建 `weights.rs` + `benchmarking.rs`，基于实际基准测试生成 weight 函数

---

### C2: StorageVersion + on_runtime_upgrade 缺失 [Critical]

**影响模块**: 全部 Entity 子模块

**现状**:
- 无任何模块声明 `#[pallet::storage_version]`
- 无任何模块实现 `on_runtime_upgrade` hook
- 经过多轮审计已有存储结构变更（如 member 的 MemberSpentUsdt、PendingMembers 值类型变更）

**主网风险**:
- 主网上线后任何存储结构变更将无法安全执行
- 缺少版本管理导致无法判断是否需要迁移
- 未迁移的旧数据可能导致 decode 失败和链 halt

**修复方案**: 
1. 所有模块添加 `#[pallet::storage_version(CURRENT_VERSION)]`
2. 创建迁移框架（参考 frame_support::migrations）
3. 当前版本设为 v1，为未来变更预留空间

---

### C3: 治理提案过期自动清理 [Critical]

**影响模块**: pallet-entity-governance

**现状**:
- 提案有 `voting_end` 字段和 `Expired` 状态
- `finalize_voting` 需要手动调用
- 无 `on_initialize` / `on_idle` 自动检查过期提案
- 如果没人调用 finalize_voting，提案永远停留在 Voting 状态
- 投票者的代币锁定（VoterTokenLocks）永远无法释放

**主网风险**:
- 用户代币被永久锁定
- EntityProposals BoundedVec 被耗尽（达到 MaxActiveProposals 上限后无法创建新提案）
- 存储无限增长

**修复方案**: 添加 `on_idle` hook，扫描活跃提案列表，自动 finalize 过期提案并释放锁定代币

---

### ~~H1: 平台费去向~~ [已确认正确]

**结论**: `do_complete_order` (line 1240-1274) 已正确实现：
- NEX 支付: `transfer_from_escrow(order_id, &seller, seller_amount)` + `transfer_from_escrow(order_id, &PlatformAccount, platform_fee)`
- Token 支付: `repatriate_reserved` 分别转给 seller 和 entity_account

**无需修复。**

---

### H2: OCW 验证器无签名验证 [High]

**影响模块**: pallet-entity-market

**现状**:
- `submit_ocw_result` (call_index 18) 使用 `ensure_none(origin)?` 接收 OCW 无签名交易
- 但代码中未见 `ValidateUnsigned` impl
- 这意味着任何人都可以提交伪造的 USDT 验证结果

**主网风险**: 攻击者可伪造 USDT 支付确认，窃取 Token

**修复方案**: 实现 `ValidateUnsigned`，验证提交者身份（签名 payload 或 authority key 验证）

---

### H3: tokensale on_initialize weight 不精确 [High]

**影响模块**: pallet-entity-tokensale

**现状**:
- `on_initialize` 扫描 ActiveRounds 自动结束过期发售
- weight 计算仅基于简单估算，无精确 DB reads/writes 计数
- 在活跃轮次数较多时可能超出区块限制

**修复方案**: 精确计算 per-round 的 weight（reads + writes + events），限制每区块处理数量

---

### H4: 治理提案执行未覆盖所有类型 [High]

**影响模块**: pallet-entity-governance

**现状**: `do_execute_proposal` 对以下 ProposalType 未实现实际链上执行：
- `TokenBurn` — 需调用 token pallet 的 burn
- `AirdropDistribution` — 需实际分发代币
- `Dividend` — 需触发分红流程
- `TreasurySpend` — 需从金库转账
- `Promotion` — 需设置折扣配置

这些类型通过验证和投票后，execute 只是记录事件但不执行实际操作。

**修复方案**: 逐个实现链上执行逻辑，或明确标记为 "off-chain execution" 类型（仅记录决议，不自动执行）

---

### M2: Shop 关闭宽限期无自动完成 [Medium]

**影响模块**: pallet-entity-shop

**现状**:
- `request_close_shop` 设置 `ShopClosingAt` 记录进入宽限期
- 宽限期满后需手动调用 `finalize_close_shop`
- 无 on_idle 自动检查

**修复方案**: 添加 on_idle 自动扫描 ShopClosingAt，宽限期满自动完成关闭

---

## 实施优先级

### Phase 1 — 发布阻塞 (Must Fix Before Mainnet)
1. **C2**: StorageVersion 声明（低工作量，高价值）
2. **C3**: 治理提案自动过期 + 代币解锁
3. **H2**: OCW ValidateUnsigned 实现
4. **H4**: 治理提案执行补全（至少 TokenBurn, TreasurySpend）

### Phase 2 — 主网上线首周 (Should Fix)
6. **C1**: 核心模块 WeightInfo（registry, order, shop, governance 优先）
7. **H3**: tokensale on_initialize weight 精确化
8. **M2**: Shop 关闭自动完成

### Phase 3 — 主网稳定后 (Nice to Have)
9. **M1**: Runtime API 查询接口
10. **M3**: 商品批量操作
11. **M4**: 24h 统计自动刷新
12. **M5**: 订单分页查询 API
13. **M6**: Vesting 自动解锁 hook
14. **C1**: 剩余模块 WeightInfo

---

## 各模块现状总结

| 子模块 | 代码行数 | WeightInfo | StorageVersion | on_initialize/on_idle | 测试覆盖 | 状态 |
|--------|---------|------------|----------------|----------------------|----------|------|
| registry | ~2300 | ❌ 硬编码 | ❌ | ❌ | 125 tests ✅ | 审计完成 |
| common | ~200 | N/A (traits) | N/A | N/A | N/A | 稳定 |
| shop | ~2000 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 审计完成 |
| product | ~200+ | ❌ 硬编码 | ❌ | ❌ | 有测试 | 基本完成 |
| order | ~1700 | ❌ 硬编码 | ❌ | ✅ on_idle | 有测试 | 审计完成 |
| review | ~389 | ✅ weights.rs | ❌ | ❌ | 有测试 | 审计完成 |
| token | ~1800 | ✅ weights.rs | ❌ | ❌ | 有测试 | 审计完成 |
| market | ~4900 | ❌ 硬编码 | ❌ | ✅ on_idle + OCW | 有测试 | 审计完成 |
| tokensale | ~2100 | ❌ 硬编码 | ❌ | ✅ on_initialize | 有测试 | 审计完成 |
| member | ~3300 | ❌ 硬编码 | ❌ | ❌ | 97 tests ✅ | 审计完成 |
| commission/core | ~2900 | ❌ 硬编码 | ❌ | ❌ | 89 tests ✅ | 审计完成 |
| commission/multi-level | ~400 | ✅ weights.rs | ❌ | ❌ | 28 tests ✅ | 审计完成 |
| commission/pool-reward | ~650 | ✅ weights.rs | ❌ | ❌ | 53 tests ✅ | 审计完成 |
| commission/referral | ~300 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 审计完成 |
| commission/level-diff | ~200 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 审计完成 |
| commission/single-line | ~300 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 审计完成 |
| commission/team | ~300 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 基本完成 |
| governance | ~1900 | ❌ 硬编码 | ❌ | ❌ 需要! | 115 tests ✅ | 审计完成 |
| disclosure | ~2100 | ❌ 硬编码 | ❌ | ❌ | 有测试 | 审计完成 |
| kyc | ~870 | ❌ 硬编码 | ❌ | ❌ | 64 tests ✅ | 审计完成 |
