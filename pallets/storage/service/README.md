# pallet-storage-service

> 路径：`pallets/storage/service/`

IPFS 存储服务核心模块，提供去中心化内容固定（Pin）、运营者管理、分层存储策略、自动计费和 OCW 健康巡检。

## 设计理念

- **多副本冗余**：多运营者节点分层存储，确保高可用
- **分层存储**：Critical / Standard / Temporary 三级策略，按数据重要性差异化配置
- **自动化**：OCW 健康巡检、故障自动修复、周期扣费、过期清理、孤儿回收
- **经济激励**：保证金锁定 + SLA 统计 + 加权奖励分配 + 阶梯惩罚 + 争议机制
- **域管理**：业务域自动 PIN 注册，按域统计和优先级调度

## 核心功能

### 1. Pin 管理

| 功能 | 说明 |
|------|------|
| 三级分层 | Critical / Standard / Temporary，按 TierConfig 配置副本数和巡检频率 |
| CID 格式校验 | 自动验证 CIDv0（`Qm` 前缀 ≥46 字节）/ CIDv1（`b`/`z`/`f` 前缀）/ 二进制格式 |
| 分层分配 | 按 StorageLayerConfig 将副本分布到 Core（Layer 1）和 Community（Layer 2）运营者 |
| 状态追踪 | Requested(0) → Pinning(1) → Pinned(2) → MarkedForUnpin |
| 续期/升级/降级 | 续期延长计费、升级 Tier 提高副本数、降级 Tier 降低费率 |
| 批量操作 | 支持批量取消 Pin（最多 20 个/次） |
| CID 锁定 | 仲裁期间锁定 CID 防止删除（治理可覆盖） |

### 2. 运营者管理

| 功能 | 说明 |
|------|------|
| 双层分类 | Core（Layer 1，项目方运营）/ Community（Layer 2，社区运营） |
| 保证金 | USD 动态计算（`DepositCalculator`），支持追加/减少；容量下限检查防止缩容到实际用量以下 |
| 状态管理 | Active(0) / Suspended(1) / Banned(2) |
| SLA 统计 | 成功率、健康度评分(0-100)、容量使用率 |
| 自动暂停 | 健康分 < 30 自动 Suspended |
| 注销宽限 | 有 Pin 时进入 7 天宽限期，OCW 迁移后退还保证金 |
| Pin 迁移 | 治理可将运营者的 Pin 分页迁移到其他运营者（cursor 分页，≤100/次） |
| Slash 争议 | 运营者可提交链上争议（≤256 字节 reason），供治理审查 |
| 奖励领取 | 从 Pool reserved 余额转入运营者账户，部分失败时发射 `RewardsClaimPartial` 事件 |

### 3. 分层策略（PinTier）

| 层级 | 默认副本 | 巡检周期 | 费率系数 | 宽限期 | 适用场景 |
|------|---------|---------|---------|-------|---------|
| Critical | 5 | 6 小时 | 1.5x | 7 天 | 证据、法律文件 |
| Standard | 3 | 24 小时 | 1.0x | 7 天 | 一般业务数据 |
| Temporary | 1 | 7 天 | 0.5x | 3 天 | 临时数据、缓存 |

### 4. 分层存储策略（StorageLayerConfig）

按 `SubjectType × PinTier` 细粒度配置副本在 Core / Community 层的分配：

```rust
pub struct StorageLayerConfig {
    pub core_replicas: u32,      // Layer 1 副本数
    pub community_replicas: u32, // Layer 2 副本数
    pub min_total_replicas: u32, // 最低副本总数
}
```

### 5. 计费机制

- **五层扣费顺序**：用户级配额 → Entity 国库（`EntityFunding` trait）→ UserFunding → IpfsPool 兜底 → 宽限期
- **扣费周期**：默认每周（100,800 块 ≈ 7 天），`on_finalize` cursor 分页处理
- **计费精度**：按 MiB 向上取整（PricePerGiBWeek / 1024），对小文件更公平
- **宽限期**：按 Tier 配置，指数退避重试（1h → 24h）
- **续期**：用户可预付 1-52 个周期续费
- **退款**：提前取消 Pin 按 BillingQueue 剩余预付时间比例退款
- **资金安全**：Pool 分配给运营者的奖励使用 `reserve` 锁定，防止超支
- **运营者激励**：按 health_score 加权分配（非等额），激励高质量服务
- **用户资金提取**：用户可随时从自己的存储资金账户提取余额

### 6. 域管理（Domain）

| 功能 | 说明 |
|------|------|
| SubjectType 自动派生 | `StoragePin::pin` 传入的 domain 字符串自动映射为域名和 SubjectType |
| 自动注册 | 业务 pallet 通过 `StoragePin` trait 自动注册域 |
| 治理管理 | 手动注册域、更新配置、设置优先级 |
| 域统计 | 按域统计 Pin 数量、存储容量、健康状态（使用 `RegisteredDomainList` 避免全表扫描） |
| 优先级调度 | OCW 按域优先级顺序巡检 |

#### 域名映射规则（SubjectType → domain）

| SubjectType | 域名 | 典型调用方 |
|-------------|------|-----------|
| Evidence | `"evidence"` | pallet-evidence |
| Product | `"product"` | pallet-entity-product |
| Entity | `"entity"` | pallet-entity-registry |
| Shop | `"shop"` | pallet-entity-shop |
| General | `"general"` | 用户直接调用 `request_pin_for_subject` |
| Custom(name) | name 内容 | 自定义域 |

### 7. on_finalize 自动化任务

每个区块结束时按优先级执行：

| 任务 | 限流 | 说明 |
|------|------|------|
| 周期扣费 | 20/块 | cursor 分页扫描 BillingQueue，五层回退扣费 |
| 健康巡检 | 10/块 | cursor 分页扫描 HealthCheckQueue，幽灵条目自动丢弃 |
| 运营者注销 | 5/块 | 检查宽限期到期，无 Pin 则退还保证金 |
| 过期 CID 清理 | 5/块 | 从 ExpiredCidQueue 出队清理关联存储（O(1)） |
| 域统计更新 | 每 24h | 遍历 RegisteredDomainList 汇总 Pin 健康状态 |

### 7.1 on_idle 孤儿 CID 扫描

利用剩余区块权重，分页扫描 `PinMeta`：

| 条件 | 判定 |
|------|------|
| `PinMeta` 存在但 `PinSubjectOf` 缺失 | 无属主的孤儿 CID |
| `PinBilling` state=2 | 已在清理队列中，跳过 |

- 每块最多 10 条，`OrphanSweepCursor` 跨块分页
- 发现孤儿后调用 `mark_cid_for_unpin`，发射 `OrphanCidDetected` 事件

### 8. OCW（Offchain Worker）

- **Pin 管理**：扫描 PendingPins，调用本地 IPFS API 执行 Pin
- **健康巡检**：GET `/api/v0/pin/ls?arg=<CID>` 检查副本状态
- **状态上报**：通过 unsigned extrinsic 提交分配/成功/失败/健康状态
- **物理删除**：调用 IPFS API 对过期 CID 执行 unpin
- **节点身份**：从 OCW 本地存储 `/memo/ipfs/node_account` 读取节点账户
- **智能分配**：`optimized_pin_allocation` 基于容量和健康度选择最优节点

## 主要类型

### SubjectType（业务域）

```rust
pub enum SubjectType {
    Evidence,           // 证据（最高优先级）
    Product,            // 商品元数据
    Entity,             // 实体元数据
    Shop,               // 店铺元数据
    General,            // 通用存储
    Custom(BoundedVec), // 自定义域
}
```

### OperatorLayer（运营者分层）

```rust
pub enum OperatorLayer {
    Core,       // Layer 1 - 项目方运营，最高优先级
    Community,  // Layer 2 - 社区运营
}
```

### PinTier（分层等级）

```rust
pub enum PinTier {
    Critical,   // 关键级
    Standard,   // 标准级（默认）
    Temporary,  // 临时级
}
```

### SubjectInfo（CID 归属信息）

```rust
pub struct SubjectInfo {
    pub subject_type: SubjectType,
    pub subject_id: u64,
}
```

### 其他核心类型（types.rs）

| 类型 | 说明 |
|------|------|
| `TierConfig` | 分层配置（replicas, health_check_interval, fee_multiplier, grace_period, enabled） |
| `StorageLayerConfig` | 分层存储策略（core_replicas, community_replicas, min_total_replicas） |
| `DomainConfig` | 域配置（auto_pin_enabled, default_tier, subject_type_id, created_at） |
| `OperatorPinHealth` | 运营者 Pin 健康统计（total/healthy/failed, health_score） |
| `OperatorMetrics` | 运营者综合指标（供 RPC/Dashboard） |
| `BillingTask` | 扣费任务（billing_period, amount_per_period, grace_status, charge_layer） |
| `GraceStatus` | 宽限状态（Normal / InGrace / Expired） |
| `ChargeLayer` | 扣费层级（IpfsPool / SubjectFunding / GracePeriod） |
| `ChargeResult` | 扣费结果（Success / EnterGrace） |
| `HealthCheckTask` | 健康巡检任务（tier, last_check, last_status, consecutive_failures） |
| `HealthStatus` | 健康状态（Healthy / Degraded / Critical / Unknown） |
| `GlobalHealthStats` | 全局健康统计 |
| `DomainStats` | 域级统计信息 |
| `LayeredPinAssignment` | CID 分层分配记录 |
| `UnpinReason` | Unpin 原因（InsufficientFunds / ManualRequest / GovernanceDecision / OperatorOffline） |
| `EntityFunding` trait | Entity 国库扣费接口（无需时配 `()`） |

## Extrinsics

### 用户接口

| 方法 | idx | 权限 | 说明 |
|------|-----|------|------|
| `request_pin_for_subject` | 10 | Signed | 为 Subject 固定 CID（含 CID 格式校验），五层扣费 |
| `fund_user_account` | 21 | Signed | 为用户资金账户充值 |
| `withdraw_user_funding` | 53 | Signed | 从用户资金账户提取余额 |
| `request_unpin` | 32 | Signed(Owner) | 取消固定 CID（按比例退款） |
| `batch_unpin` | 48 | Signed(Owner) | 批量取消 Pin（≤20 个） |
| `renew_pin` | 45 | Signed(Owner) | 续期 Pin（1-52 个周期） |
| `upgrade_pin_tier` | 46 | Signed(Owner) | 升级 Pin 分层等级 |
| `downgrade_pin_tier` | 54 | Signed(Owner) | 降级 Pin 分层等级（Critical→Standard→Temporary） |
| `fund_ipfs_pool` | 44 | Signed | 向 IPFS 公共池充值 |
| `fund_subject_account` | 9 | Signed | **已废弃**，立即返回 BadParams |

### 运营者接口

| 方法 | idx | 权限 | 说明 |
|------|-----|------|------|
| `join_operator` | 3 | Signed | 注册运营者，锁定保证金 |
| `update_operator` | 4 | Signed | 更新运营者元信息（含容量下限检查） |
| `leave_operator` | 5 | Signed | 注销运营者（宽限期机制） |
| `pause_operator` | 22 | Signed | 暂停接单 |
| `resume_operator` | 23 | Signed | 恢复接单 |
| `report_probe` | 7 | Signed | 上报心跳 |
| `operator_claim_rewards` | 16 | Signed | 领取奖励（部分失败发射 `RewardsClaimPartial`） |
| `top_up_bond` | 49 | Signed | 追加保证金 |
| `reduce_bond` | 50 | Signed | 减少保证金 |
| `dispute_slash` | 55 | Signed | 对 slash 发起争议（≤256 字节 reason） |
| `mark_pinned` | 1 | Signed | OCW 上报 Pin 成功 |
| `mark_pin_failed` | 2 | Signed | OCW 上报 Pin 失败 |

### OCW Unsigned 接口

| 方法 | idx | 说明 |
|------|-----|------|
| `ocw_mark_pinned` | 40 | 上报 Pin 成功 |
| `ocw_mark_pin_failed` | 41 | 上报 Pin 失败 |
| `ocw_submit_assignments` | 42 | 提交分层 Pin 分配 |
| `ocw_report_health` | 43 | 上报健康巡检结果 |

### 公共清理接口

| 方法 | idx | 说明 |
|------|-----|------|
| `cleanup_expired_cids` | 34 | 清理过期 CID（从 ExpiredCidQueue 出队） |
| `cleanup_expired_locks` | 51 | 清理过期 CID 锁（cursor 分页，≤20 个） |

### 治理接口

| 方法 | idx | 说明 |
|------|-----|------|
| `set_operator_status` | 6 | 设置运营者状态 |
| `slash_operator` | 8 | 惩罚运营者保证金 |
| `set_billing_params` | 12 | 设置计费参数 |
| `distribute_to_operators` | 13 | 分配收益给运营者 |
| `update_tier_config` | 15 | 更新分层配置 |
| `emergency_pause_billing` | 17 | 紧急暂停自动扣费 |
| `resume_billing` | 18 | 恢复自动扣费 |
| `set_storage_layer_config` | 19 | 设置分层存储策略 |
| `set_operator_layer` | 20 | 设置运营者层级 |
| `register_domain` | 25 | 注册业务域（自动加入 RegisteredDomainList） |
| `update_domain_config` | 26 | 更新域配置 |
| `set_domain_priority` | 27 | 设置域巡检优先级 |
| `governance_force_unpin` | 33 | 强制下架 CID |
| `migrate_operator_pins` | 47 | 迁移运营者 Pin（cursor 分页，≤100 个） |

## Trait 接口

### StoragePin（统一存储 Pin 接口）

```rust
pub trait StoragePin<AccountId> {
    fn pin(
        owner: AccountId,
        domain: &[u8],          // "evidence", "product", "entity", "shop" 等
        subject_id: u64,
        entity_id: Option<u64>, // 所属 Entity（用于 Entity 国库扣费层）
        cid: Vec<u8>,
        size_bytes: u64,        // 实际文件大小（字节），用于精确计费
        tier: PinTier,
    ) -> DispatchResult;

    fn unpin(owner: AccountId, cid: Vec<u8>) -> DispatchResult;
}
```

### CidLockManager（仲裁锁定 CID）

```rust
pub trait CidLockManager<Hash, BlockNumber> {
    fn lock_cid(cid_hash: Hash, reason: Vec<u8>, until: Option<BlockNumber>) -> DispatchResult;
    fn unlock_cid(cid_hash: Hash, reason: Vec<u8>) -> DispatchResult;
    fn is_locked(cid_hash: &Hash) -> bool;
}
```

## 存储项

### Pin 管理

| 存储 | 类型 | 说明 |
|------|------|------|
| `PendingPins` | Map(Hash → tuple) | Pin 订单（payer, replicas, subject_id, size, deposit） |
| `PinMeta` | Map(Hash → PinMetadata) | 副本数、大小、时间 |
| `PinStateOf` | Map(Hash → u8) | 0=Requested, 1=Pinning, 2=Pinned, 3=Degraded, 4=Failed |
| `PinAssignments` | Map(Hash → BoundedVec\<AccountId, 16\>) | 副本运营者 |
| `PinSuccess` | DoubleMap(Hash, AccountId → bool) | 成功标记 |
| `PinSubjectOf` | Map(Hash → (AccountId, u64)) | CID → owner 映射 |
| `CidToSubject` | Map(Hash → BoundedVec\<SubjectInfo, 8\>) | CID → Subject |
| `CidTier` | Map(Hash → PinTier) | CID 分层等级 |
| `CidRegistry` | Map(Hash → BoundedVec\<u8, 128\>) | CID 明文 |
| `CidEntityOf` | Map(Hash → u64) | CID → Entity ID |
| `OwnerPinIndex` | Map(AccountId → BoundedVec\<Hash, 1000\>) | 用户 CID 索引 |
| `CidLocks` | Map(Hash → (reason, Option\<BlockNumber\>)) | 仲裁锁 |
| `CidUnpinReason` | Map(Hash → UnpinReason) | Unpin 原因 |

### 运营者管理

| 存储 | 类型 | 说明 |
|------|------|------|
| `Operators` | Map(AccountId → OperatorInfo) | 运营者注册表 |
| `OperatorBond` | Map(AccountId → Balance) | 保证金 |
| `OperatorUsedBytes` | Map(AccountId → u64) | 实际存储字节数 |
| `OperatorSla` | Map(AccountId → SlaStats) | SLA 统计 |
| `OperatorPinStats` | Map(AccountId → OperatorPinHealth) | Pin 健康统计 |
| `OperatorPinCount` | Map(AccountId → u32) | Pin 数量（O(1)） |
| `OperatorRewards` | Map(AccountId → Balance) | 待提取奖励 |
| `ActiveOperatorIndex` | Value(BoundedVec\<AccountId, 256\>) | 活跃索引 |
| `PendingUnregistrations` | Map(AccountId → BlockNumber) | 注销宽限期 |

### 计费系统

| 存储 | 类型 | 说明 |
|------|------|------|
| `PricePerGiBWeek` | Value(u128) | 每 GiB·周单价 |
| `BillingPeriodBlocks` | Value(u32) | 扣费周期 |
| `GraceBlocks` | Value(u32) | 宽限期（按 Tier 覆盖） |
| `MaxChargePerBlock` | Value(u32) | 每块最大扣费数 |
| `BillingPaused` | Value(bool) | 计费暂停开关 |
| `PinBilling` | Map(Hash → (BlockNumber, u128, u8)) | CID 计费状态（0=Active, 1=Grace, 2=Expired） |
| `BillingQueue` | DoubleMap(BlockNumber, Hash → BillingTask) | 扣费队列 |
| `BillingSettleCursor` | Value(BlockNumber) | 计费游标 |
| `CidBillingDueBlock` | Map(Hash → BlockNumber) | CID → due_block 反向索引 |
| `ExpiredCidPending` | Value(bool) | 有无待清理过期 CID |
| `ExpiredCidQueue` | Value(BoundedVec\<Hash, 200\>) | 待清理 CID 队列 |

### 配额与资金

| 存储 | 类型 | 说明 |
|------|------|------|
| `PublicFeeQuotaUsage` | Map(AccountId → (used, reset_block)) | 用户级配额 |
| `UserFundingBalance` | Map(AccountId → Balance) | 用户资金余额 |
| `TotalChargedFromPool` | Value(Balance) | 累计池扣款 |
| `TotalChargedFromSubject` | Value(Balance) | 累计用户扣款 |
| `SubjectUsage` | Map((AccountId, domain, subject_id) → Balance) | Subject 用量 |
| `SubjectMinReserve` | Value(Balance) | Subject 最低储备金 |

### 域、分层、健康

| 存储 | 类型 | 说明 |
|------|------|------|
| `PinTierConfig` | Map(PinTier → TierConfig) | Tier 配置 |
| `StorageLayerConfigs` | Map((SubjectType, PinTier) → StorageLayerConfig) | 分层策略 |
| `LayeredPinAssignments` | Map(Hash → LayeredPinAssignment) | CID 分层分配 |
| `DomainPins` | DoubleMap(domain, Hash → ()) | 域 Pin 索引 |
| `RegisteredDomains` | Map(domain → DomainConfig) | 域配置 |
| `RegisteredDomainList` | Value(BoundedVec\<domain, 128\>) | 已注册域列表（避免全表扫描） |
| `DomainHealthStats` | Map(domain → DomainStats) | 域健康统计 |
| `DomainPriority` | Map(domain → u8) | 域优先级 |
| `HealthCheckQueue` | DoubleMap(BlockNumber, Hash → HealthCheckTask) | 巡检队列 |
| `HealthCheckStats` | Value(GlobalHealthStats) | 全局健康统计 |
| `MinReplicasThreshold` | Value(u32) | 最小副本数 |

### 分页游标

| 存储 | 说明 |
|------|------|
| `BillingSettleCursor` | 计费扫描分页 |
| `HealthCheckSettleCursor` | 健康巡检分页 |
| `OrphanSweepCursor` | 孤儿 CID 扫描分页 |
| `LockCleanupCursor` | 过期锁清理分页 |
| `MigrateOpCursor` | 运营者 Pin 迁移分页 |

## 配置参数（Config）

### 常量

| 参数 | 说明 | 默认值 |
|------|------|-------|
| `MinOperatorBond` | 最小保证金（NEX 兜底值） | 100 UNIT |
| `MinOperatorBondUsd` | 最小保证金（USD, 精度 10^6） | 100 USDT |
| `MinCapacityGiB` | 运营者最小容量 | 10 GiB |
| `MaxCidHashLen` | CID 哈希最大长度 | 8192 |
| `MaxPeerIdLen` | PeerId 最大长度 | 32 |
| `SubjectPalletId` | 派生子账户 PalletId | — |
| `MonthlyPublicFeeQuota` | 每月公共配额 | 10 NEX |
| `QuotaResetPeriod` | 配额重置周期 | 432,000 块(~30 天) |
| `DefaultBillingPeriod` | 默认扣费周期 | 432,000 块(~30 天) |
| `OperatorGracePeriod` | 运营者注销宽限期 | 100,800 块(~7 天) |

### 外部类型

| 参数 | 说明 |
|------|------|
| `Currency` | 货币接口（Currency + ReservableCurrency） |
| `Balance` | 余额类型（AtLeast32BitUnsigned） |
| `GovernanceOrigin` | 治理 Origin |
| `DepositCalculator` | 保证金动态计算器（USD → NEX） |
| `FeeCollector` | 费用接收账户 |
| `EntityFunding` | Entity 国库扣费接口（由 entity-registry 实现，无需时配 `()`） |
| `WeightInfo` | 权重信息 |

### 关键账户

| 参数 | 说明 |
|------|------|
| `IpfsPoolAccount` | IPFS 公共池 |
| `OperatorEscrowAccount` | 运营者托管账户 |

## 错误码

| 错误 | 说明 |
|------|------|
| `BadParams` | 参数非法 |
| `OrderNotFound` | CID/订单不存在 |
| `BadStatus` | 无效状态 |
| `NotOwner` | 非所有者 |
| `InvalidCidFormat` | CID 格式无效（长度/前缀不合法） |
| `OperatorNotFound` | 运营者不存在 |
| `OperatorExists` | 运营者已存在 |
| `OperatorBanned` | 运营者已被禁用 |
| `AlreadyPaused` / `NotPaused` | 暂停/恢复状态冲突 |
| `InsufficientBond` | 保证金不足 |
| `InsufficientCapacity` | 容量不足 |
| `HasActiveAssignments` | 仍有未完成的副本分配 |
| `OperatorNotAssigned` | 未被指派到该 CID |
| `CidAlreadyPinned` | CID 已被 Pin |
| `NoActiveOperators` | 无活跃运营者 |
| `NoRewardsAvailable` | 无可用奖励 |
| `TierConfigNotFound` | 分层配置未找到 |
| `InvalidReplicas` | 副本数无效（1-10） |
| `IntervalTooShort` | 巡检间隔太短（≥600 块） |
| `InvalidMultiplier` | 费率系数无效 |
| `InsufficientUserFunding` | 用户资金账户余额不足 |
| `InvalidTierDowngrade` | 无效的 Tier 降级方向 |
| `DomainTooLong` | 域名超过 32 字节 |
| `DomainAlreadyExists` | 域已存在 |
| `DomainNotFound` | 域不存在 |
| `DomainPinDisabled` | 域的自动 PIN 已禁用 |

## 事件

### Pin 生命周期

| 事件 | 说明 |
|------|------|
| `PinRequested` | 请求已受理 |
| `PinMarkedPinned` / `PinMarkedFailed` | Pin 成功/失败 |
| `PinStateChanged` | 状态迁移 |
| `PinCharged` | 完成周期扣费 |
| `PinGrace` / `PinExpired` | 进入宽限/超过宽限 |
| `PinRenewed` | Pin 已续期 |
| `PinTierUpgraded` / `PinTierDowngraded` | Tier 升级/降级 |
| `MarkedForUnpin` | 标记为待 Unpin |
| `UnpinRefund` | 提前取消退款 |
| `BatchUnpinCompleted` | 批量取消完成 |
| `OrphanCidDetected` | 检测到孤儿 CID |
| `PinRemoved` | CID 已物理删除 |

### 运营者

| 事件 | 说明 |
|------|------|
| `OperatorJoined` / `OperatorUpdated` / `OperatorLeft` | 注册/更新/离开 |
| `OperatorUnregistered` | 注销完成（保证金退还） |
| `OperatorStatusChanged` | 状态变更 |
| `OperatorSlashed` | 保证金被扣罚 |
| `SlashDisputed` | 运营者发起 slash 争议（operator, amount, reason） |
| `OperatorAutoSuspended` | 健康分过低自动暂停 |
| `OperatorPinsMigrated` | Pin 迁移完成 |
| `BondTopUp` / `BondReduced` | 保证金追加/减少 |
| `RewardsClaimed` | 领取奖励 |
| `RewardsClaimPartial` | 部分领取成功（Pool 余额不足时） |

### 计费

| 事件 | 说明 |
|------|------|
| `ChargedFromIpfsPool` | 从池扣费 |
| `ChargedFromSubjectFunding` | 从用户资金扣费 |
| `ChargedFromEntityTreasury` | 从 Entity 国库扣费 |
| `UserFunded` | 用户资金充值 |
| `UserFundingWithdrawn` | 用户资金提取 |
| `IpfsPoolFunded` | 公共池充值 |
| `IpfsPoolLowBalanceWarning` | 池余额告警 |
| `BillingPausedByGovernance` / `BillingResumedByGovernance` | 计费暂停/恢复 |
| `RewardDistributed` | 奖励分配完成 |

### 域管理

| 事件 | 说明 |
|------|------|
| `DomainRegistered` | 域已注册 |
| `DomainConfigUpdated` | 域配置更新 |
| `DomainPrioritySet` | 域优先级设置 |
| `DomainStatsUpdated` | 域统计更新 |
| `GovernanceForceUnpinned` | 治理强制下架 |

### CID 锁定

| 事件 | 说明 |
|------|------|
| `CidLocked` | CID 已锁定 |
| `CidUnlocked` | CID 已解锁 |

## ValidateUnsigned

仅接受 `Local` 或 `InBlock` 来源的 OCW unsigned 交易：

| Call | 验证规则 |
|------|---------|
| `ocw_mark_pinned` | CID 在 PendingPins 中 + 运营者 Active + 防重放 |
| `ocw_mark_pin_failed` | 同上 |
| `ocw_submit_assignments` | CID 在 PendingPins 中 + 无现有分配 + 防重放 |
| `ocw_report_health` | 运营者 Active + CID 存在 + 运营者已分配到该 CID + 防重放 |

## 集成示例

### StoragePin（所有业务 pallet 统一使用）

```rust
pub trait Config: frame_system::Config {
    type StoragePin: StoragePin<Self::AccountId>;
}

// size_bytes: 实际文件大小（字节），用于精确计费
T::StoragePin::pin(who, b"evidence", evidence_id, None, cid_vec, file_size, PinTier::Critical)?;
T::StoragePin::unpin(who, cid_vec)?;
```

### CidLockManager（仲裁场景）

```rust
pub trait Config: frame_system::Config {
    type CidLockManager: CidLockManager<Self::Hash, BlockNumberFor<Self>>;
}

T::CidLockManager::lock_cid(cid_hash, b"arbitration:123".to_vec(), Some(expiry))?;
```

## RPC 查询接口

| 方法 | 说明 |
|------|------|
| `get_domain_stats(domain)` | 查询指定域的统计信息 |
| `get_all_domain_stats()` | 查询所有域统计（按优先级排序） |
| `get_domain_cids(domain, offset, limit)` | 分页查询域的 CID 列表 |
| `get_user_cids(user)` | 查询用户的所有 CID 及元数据 |

## 存储迁移

| 版本 | 说明 |
|------|------|
| v0 → v1 | 从 `RegisteredDomains` 填充 `RegisteredDomainList`（一次性迁移） |

## 文件结构

| 文件 | 说明 |
|------|------|
| `src/lib.rs` | 主模块：Config、Storage、Events、Errors、Extrinsics、Hooks |
| `src/types.rs` | 类型定义：SubjectType、PinTier、TierConfig、BillingTask 等 |
| `src/migrations.rs` | 存储迁移（v1: RegisteredDomainList） |
| `src/tests.rs` | 单元测试（36 个） |

## 审计历史

### Round 7（2026-03，深度审计 + 全量修复）

**过度设计移除（5 项）**

| ID | 说明 | 状态 |
|----|------|------|
| O1 | 移除 External/Layer3 占位设计（OperatorLayer、StorageLayerConfig） | ✅ |
| O2 | 删除 ChargeStrategy 死代码枚举 | ✅ |
| O3 | 删除 PricingParams 空壳存储 | ✅ |
| O4 | 简化 SubjectInfo（移除 funding_share） | ✅ |
| O5 | 移除 DomainConfig::owner_pallet 未使用字段 | ✅ |

**冗余清理（4 项已完成，2 项暂缓）**

| ID | 说明 | 状态 |
|----|------|------|
| R3 | 移除断链的 DueQueue + DueEnqueueSpread + enqueue_due | ✅ |
| R4 | 删除 ReplicasForLevel0~3 + set_replicas_config + get_recommended_replicas | ✅ |
| R5 | 提取 do_cleanup_single_cid 辅助函数消除重复清理代码 | ✅ |
| R6 | fund_subject_account 硬阻断（已废弃） | ✅ |
| R1 | 合并 PinAssignments 到 LayeredPinAssignments | 📌 暂缓 |
| R2 | 移除 SimplePinAssignments/SimpleNodeStatsMap 影子系统 | 📌 暂缓 |

**必需功能新增（10 项）**

| ID | 级别 | 说明 | 状态 |
|----|------|------|------|
| F1 | Critical | StoragePin::pin() 增加 size_bytes 参数解决 CID 大小估算不准确 | ✅ |
| F2 | High | 新增 withdraw_user_funding extrinsic | ✅ |
| F3 | High | 新增 downgrade_pin_tier extrinsic | ✅ |
| F4 | High | CID 格式基本校验（validate_cid） | ✅ |
| F5 | High | get_user_cids RPC 查询函数 | ✅ |
| F6 | Medium | update_operator 容量下限检查 | ✅ |
| F7 | Medium | dispute_slash 争议机制 | ✅ |
| F8 | Medium | operator_claim_rewards 部分失败事件 | ✅ |
| F9 | Medium | 存储迁移基础设施（migrations.rs + STORAGE_VERSION） | ✅ |
| F10 | Medium | RegisteredDomainList 避免全表扫描 | ✅ |

**安全加固（P0）**

| ID | 说明 | 状态 |
|----|------|------|
| P0-1 | 10 处 BlockNumber 原生 `+` → `saturating_add` | ✅ |
| P0-2 | cleanup_expired_locks cursor 分页（LockCleanupCursor） | ✅ |
| P0-3 | migrate_operator_pins cursor 分页（MigrateOpCursor） | ✅ |
| P0-4 | update_domain_health_stats_impl 改用 RegisteredDomainList 替代全表扫描 | ✅ |

### 历史审计轮次

| 轮次 | 时间 | 关键修复 |
|------|------|---------|
| R4 | 2026-03 | renew_pin/upgrade_pin_tier 收费不延长/不更新 BillingQueue |
| R5 | 2026-03 | Pool 资金安全（reserve 锁定）、MiB 计费精度、用户级配额 |
| R6 | 2026-03 | Entity 封禁级联 unpin、健康检查幽灵条目、孤儿 CID 回收 |

## 相关模块

- `pallets/entity/registry/` — 实体注册（实现 EntityFunding、调用 StoragePin）
- `pallets/entity/product/` — 商品（调用 StoragePin）
- `pallets/entity/shop/` — 店铺（调用 StoragePin）
- `pallets/dispute/evidence/` — 证据存证（调用 StoragePin）
- `pallets/dispute/arbitration/` — 仲裁（调用 CidLockManager）
