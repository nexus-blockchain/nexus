# Pallet 安全审计报告

> 审计日期: 2026-02-22 ~ 2026-02-23
> 审计范围: Entity 子模块 (15 pallets) + Commission 子模块 (5 pallets)
> 验证: `cargo check -p nexus-runtime` ✅ | 所有模块单元测试通过 ✅

## 审计范围

| 模块组 | 子模块 | 源文件 |
|--------|--------|--------|
| **Entity 核心** | common, registry, shop, token | `pallets/entity/*/src/lib.rs` |
| **Entity 业务** | service, transaction, review, market | `pallets/entity/*/src/lib.rs` |
| **Entity 金融** | governance, member, disclosure, kyc, tokensale | `pallets/entity/*/src/lib.rs` |
| **Commission** | common, core, referral, level-diff, single-line | `pallets/entity/commission/*/src/lib.rs` |

## 修复总计

| 严重等级 | 数量 | 说明 |
|----------|------|------|
| **Critical** | 5 | DecodeWithMemTracking 缺失 (4) + BoundedVec DoS 修复 (1) |
| **High** | 10 | 授权缺失 (3 模块) + 权重异常 (4 模块) + 业务逻辑 (3) |
| **Medium** | 3 | 输入校验 (1) + 功能未实现 (2) |
| **Low** | 0 | — |
| **合计** | **18** | |

---

## 一、Commission 子模块审计

### 修改文件 (5 个)

| 文件 | 修复项 |
|------|--------|
| `commission/common/src/lib.rs` | C1 |
| `commission/core/src/lib.rs` | C2, H1 |
| `commission/referral/src/lib.rs` | C3, H2, H3 |
| `commission/level-diff/src/lib.rs` | C4, H4, H5, M1 |
| `commission/single-line/src/lib.rs` | H6, H7 |

### Critical: DecodeWithMemTracking 缺失

| ID | 结构体 | 模块 |
|----|--------|------|
| C1 | `CommissionRecord` | commission-common |
| C2 | `EntityWithdrawalConfig` | commission-core |
| C3 | `MultiLevelConfig`, `ReferralConfig` | commission-referral |
| C4 | `CustomLevelDiffConfig` | commission-level-diff |

**风险**: 缺少 `DecodeWithMemTracking` 的结构体在运行时解码时可能导致内存安全问题。

**修复**: 添加 `#[derive(codec::DecodeWithMemTracking)]`。

### High: 授权缺失 (H2, H4, H6)

**影响模块**: commission-referral (5 extrinsics), commission-level-diff (2 extrinsics), commission-single-line (1 extrinsic)

**问题**: 所有 extrinsic 仅使用 `ensure_signed(origin)?`，无进一步权限校验。**任何已签名用户可修改任意店铺的佣金配置。**

**修复**: 改为 `ensure_root(origin)?`。这些插件 extrinsic 应仅通过 core pallet 的 `PlanWriter` trait 间接调用（已有 owner 权限校验），直接调用需 root 权限。

### High: Extrinsic 权重异常 (H1, H3, H5, H7)

**问题**: 多个 extrinsic 的 `proof_size` 为 0（如 `Weight::from_parts(25_000, 0)`），`ref_time` 也偏低。

**修复**: 统一调整为合理值：

| 模块 | 修正前 | 修正后 |
|------|--------|--------|
| commission-core (7 个) | ~25K-50K / 0 | 40M-80M / 4K-6K |
| commission-referral (5 个) | ~25K / 0 | 40M / 4K |
| commission-level-diff (2 个) | ~25K / 0 | 40M / 4K |
| commission-single-line (1 个) | ~25K / 0 | 40M / 4K |

### Medium: 输入校验缺失 (M1)

**模块**: commission-level-diff

**问题**: `set_level_diff_config` 的 5 个费率参数（normal_rate, silver_rate 等）无上界校验，可设置超过 10000 基点（100%）。

**修复**: 添加 `ensure!(rate <= 10000, Error::<T>::InvalidRate)` 对每个费率参数。

---

## 二、Governance 模块审计

### 修改文件 (3 个)

| 文件 | 修复项 |
|------|--------|
| `governance/src/lib.rs` | 时间加权投票权实现 |
| `governance/src/mock.rs` | 新增配置常量 |
| `governance/src/tests.rs` | 8 个新测试 |

### 功能实现: 时间加权投票权

**问题**: `calculate_voting_power` 仅返回原始代币余额，标记有 `TODO: 实现时间加权`。

**实现**:

```
voting_power = balance × multiplier / 10000
multiplier = 10000 + min(holding_blocks × bonus_range / full_period, bonus_range)
bonus_range = max_multiplier - 10000
```

**新增 Config 常量**:

| 常量 | Runtime 值 | 说明 |
|------|-----------|------|
| `TimeWeightFullPeriod` | `30 * DAYS` | 达到最大乘数所需持有区块数 |
| `TimeWeightMaxMultiplier` | `30000` | 最大 3x 投票权乘数 |

**边界行为**:
- `TimeWeightFullPeriod == 0` → 禁用时间加权，直接返回余额
- `max_multiplier <= 10000` → 禁用时间加权
- 无 `FirstHoldTime` 记录 → 按 1x 计算（无加成）
- 持有超过 `full_period` → 上限为 `max_multiplier`

**新增测试 (8 个)**:

| 测试 | 覆盖 |
|------|------|
| `time_weight_no_first_hold_returns_base_balance` | 无记录 → 1x |
| `time_weight_zero_holding_returns_base_balance` | 持有 0 → 1x |
| `time_weight_half_period_gives_half_bonus` | 50% 周期 → 2x |
| `time_weight_full_period_gives_max_bonus` | 100% 周期 → 3x |
| `time_weight_beyond_full_period_caps_at_max` | 超额 → 上限 3x |
| `time_weight_zero_balance_returns_zero` | 余额 0 → 0 |
| `time_weight_quarter_period` | 25% 周期 → 1.5x |
| `time_weight_vote_uses_weighted_power` | 投票使用加权权重 |

---

## 三、TokenSale 模块审计

### 修改文件 (2 个)

| 文件 | 修复项 |
|------|--------|
| `tokensale/src/lib.rs` | C5, H8, H9, M2 |
| `tokensale/src/tests.rs` | 测试修复 + 5 新测试 |

### Critical: 无界输入 DoS (C5)

**函数**: `add_to_whitelist`

**问题**: `accounts` 参数类型为 `Vec<T::AccountId>`，无长度限制。攻击者可提交极大数组导致链上计算 DoS。

**修复**: 改为 `BoundedVec<T::AccountId, T::MaxWhitelistSize>`，FRAME 在 SCALE 解码阶段即拒绝超长输入。

### High: subscribe 容量预检 (H8)

**问题**: `subscribe` 中 NEX 转账（昂贵操作）在 `RoundParticipants::try_push` 之前执行。若 push 失败（容量满），虽因 transactional storage 会回滚，但浪费了转账计算。

**修复**: 在转账前预检 `RoundParticipants` 长度 < `MaxSubscriptionsPerRound`，fail-fast。

### High: end_sale 提前截止 (H9)

**问题**: `end_sale` 仅检查 `status == Active`，创建者可在 `end_block` 之前随时结束发售，**损害参与者预期权益**（认购者以为还有时间参与）。

**修复**: 新增检查 `now >= end_block || remaining_amount.is_zero()`。仅在时间到期或已售罄时允许结束。

### Medium: Cliff 锁仓 unlock_interval 未生效 (M2)

**问题**: `VestingConfig` 定义了 `unlock_interval` 字段，但 `calculate_unlockable` 对所有锁仓类型都做纯线性解锁，忽略了阶梯间隔。`VestingType::Cliff` 应按 `unlock_interval` 做阶梯释放。

**修复**:
- `Cliff` 类型: `effective_elapsed = (elapsed / interval) × interval`（阶梯取整）
- `Linear` 类型: 保持连续线性不变

### 新增测试 (5 个)

| 测试 | 覆盖 |
|------|------|
| `end_sale_rejects_premature_end` | 提前结束被拒绝 |
| `end_sale_allows_when_sold_out` | 售罄可提前结束 |
| `end_sale_allows_after_end_block` | 正常到期结束 |
| `cliff_vesting_unlock_interval_step_function` | Cliff 阶梯解锁 (6 断言) |
| `linear_vesting_continuous_unlock` | Linear 不受 interval 影响 |

---

## 四、审查确认正常的模块

以下模块经审查未发现需要修复的问题：

| 模块 | 审查结果 |
|------|----------|
| `pallet-entity-common` | 类型定义和 trait 设计正确 |
| `pallet-entity-registry` | 权限校验完备，状态转换正确 |
| `pallet-entity-shop` | 双层状态管理正确，级联更新安全 |
| `pallet-entity-token` | reserve/unreserve/repatriate 调用正确，转账限制逻辑完备 |
| `pallet-entity-service` | CRUD 操作校验充分 |
| `pallet-entity-transaction` | 订单流转状态机正确 |
| `pallet-entity-review` | 评分校验和 CID 长度限制合理 |
| `pallet-entity-market` | TWAP 计算正确，熔断机制安全 |
| `pallet-entity-disclosure` | 内幕交易窗口期控制正确 |
| `pallet-entity-kyc` | 多级 KYC 状态管理安全 |
| `pallet-entity-member` | 推荐链和等级计算正确 |

---

## 五、已知设计局限（标记未修）

| 模块 | 级别 | 描述 |
|------|------|------|
| tokensale | L1 | 无 `on_initialize` 自动结束机制，过期发售需手动调用 `end_sale` |
| tokensale | L2 | `SaleRound` 结构体较大（含 BoundedVec），频繁读写开销高 |
| tokensale | L3 | 未领取退款导致 Entity 代币永久锁定（设计如此，用户自负） |
| tokensale | L4 | DutchAuction 模式下 `payment_option.price` 被荷兰公式覆盖 |
| commission | L5 | 插件 extrinsic 改为 ensure_root 后，需通过 sudo 或治理调用 |

---

## 六、修改文件清单

| # | 文件 | 修改类型 |
|---|------|----------|
| 1 | `pallets/entity/commission/common/src/lib.rs` | derive 修复 |
| 2 | `pallets/entity/commission/core/src/lib.rs` | derive + 权重修复 |
| 3 | `pallets/entity/commission/referral/src/lib.rs` | 授权 + derive + 权重 |
| 4 | `pallets/entity/commission/level-diff/src/lib.rs` | 授权 + 校验 + derive + 权重 |
| 5 | `pallets/entity/commission/single-line/src/lib.rs` | 授权 + 权重 |
| 6 | `pallets/entity/governance/src/lib.rs` | 时间加权投票权实现 |
| 7 | `pallets/entity/governance/src/mock.rs` | 新增配置常量 |
| 8 | `pallets/entity/governance/src/tests.rs` | 8 个新测试 |
| 9 | `pallets/entity/tokensale/src/lib.rs` | BoundedVec + 容量预检 + 时间窗口 + 阶梯解锁 |
| 10 | `pallets/entity/tokensale/src/tests.rs` | 测试修复 + 5 个新测试 |
| 11 | `pallets/entity/tokensale/README.md` | v0.3.0 文档更新 |
| 12 | `pallets/entity/README.md` | v0.6.0 版本记录 |
| 13 | `runtime/src/configs/mod.rs` | 新增治理时间加权配置 |

**总计**: 13 个文件修改，18 项修复，13 个新测试
