# Storage Pallets

存储服务模块组，负责 IPFS 去中心化存储管理和数据生命周期治理。提供从内容固定、运营者管理、自动计费到分级归档的完整存储基础设施。

## 模块结构

```
storage/
├── service/     # 存储服务核心 (pallet-storage-service)
│   ├── src/
│   │   ├── lib.rs          # 主逻辑（Config, Storage, Extrinsics, Events, Errors, OCW）
│   │   ├── types.rs        # 核心类型定义（SubjectType, PinTier, TierConfig, OperatorLayer 等）
│   │   ├── weights.rs      # WeightInfo trait + 基准权重（27 函数）
│   │   ├── runtime_api.rs  # Runtime API（用户资金查询）
│   │   ├── tests.rs        # 单元测试
│   │   └── benchmarking.rs # 基准测试
│   └── Cargo.toml
│
└── lifecycle/   # 存储生命周期管理 (pallet-storage-lifecycle)
    ├── src/
    │   ├── lib.rs          # 主逻辑（ArchivableData trait, 分级归档引擎, Extrinsics）
    │   ├── runtime_api.rs  # Runtime API（归档仪表盘查询）
    │   ├── mock.rs         # 测试 Mock
    │   └── tests.rs        # 单元测试
    └── Cargo.toml
```

---

## pallet-storage-service（存储服务核心）

### 概述

IPFS Pin 管理的核心 pallet，提供内容固定、运营者管理、分层存储、自动计费与健康巡检功能。通过 Offchain Worker (OCW) 与本地 IPFS Cluster API 交互执行物理 Pin/Unpin 操作。

### 公共 Trait 接口

| Trait | 说明 |
|-------|------|
| `IpfsPinner<AccountId, Balance>` | IPFS 自动 Pin 接口，供业务 pallet 调用。支持四层扣费回退机制（IpfsPool → SubjectFunding → Caller → GracePeriod） |
| `ContentRegistry` | 内容注册接口，新业务 pallet 一行代码即可完成域注册 + CID 固定 + 自动扣费 |
| `CidLockManager<Hash, BlockNumber>` | CID 锁定接口，仲裁期间锁定证据 CID 防止被删除 |
| `SubjectOwnerProvider<AccountId>` | Subject 所有者只读提供者，用于权限检查（低耦合设计） |

### 核心类型（`types.rs`）

#### Subject 管理

| 类型 | 说明 |
|------|------|
| `SubjectType` | 业务域枚举：`Evidence`(0), `OtcOrder`(1), `Chat`(5), `Livestream`(6), `Swap`(7), `Arbitration`(8), `UserProfile`(9), `Product`(10), `General`(98), `Custom`(99) |
| `SubjectInfo` | CID 归属信息（subject_type, subject_id, funding_share） |
| `DomainConfig` | 域配置（auto_pin_enabled, default_tier, subject_type_id, owner_pallet, created_at） |

#### Pin 分层

| 类型 | 说明 |
|------|------|
| `PinTier` | Pin 等级枚举：`Critical`（5副本/6h巡检/1.5x费率）、`Standard`（3副本/24h/1.0x）、`Temporary`（1副本/7d/0.5x） |
| `TierConfig` | 分层配置参数（replicas, health_check_interval, fee_multiplier, grace_period_blocks, enabled） |

#### 运营者分层

| 类型 | 说明 |
|------|------|
| `OperatorLayer` | 运营者层级：`Core`（Layer 1 项目方）、`Community`（Layer 2 社区）、`External`（Layer 3 外部网络，预留） |
| `StorageLayerConfig` | 分层存储策略（core_replicas, community_replicas, allow_external, min_total_replicas），按 SubjectType 预设不同配置 |
| `LayeredPinAssignment<AccountId>` | CID 分层 Pin 分配记录（core_operators, community_operators, external_used） |
| `OperatorMetrics<Balance, BlockNumber>` | 运营者综合指标（供 RPC 返回） |

#### 健康巡检

| 类型 | 说明 |
|------|------|
| `HealthCheckTask<BlockNumber>` | 巡检任务（tier, last_check, last_status, consecutive_failures） |
| `HealthStatus` | 健康状态：`Healthy`, `Degraded`, `Critical`, `Unknown` |
| `GlobalHealthStats<BlockNumber>` | 全局健康统计（total_pins, healthy_count, degraded_count, critical_count 等） |
| `DomainStats` | 域级健康统计 |
| `OperatorPinHealth<BlockNumber>` | 运营者 Pin 健康（含健康度评分 0-100） |

#### 计费系统

| 类型 | 说明 |
|------|------|
| `BillingTask<BlockNumber, Balance>` | 扣费任务（billing_period, amount_per_period, last_charge, grace_status, charge_layer） |
| `GraceStatus<BlockNumber>` | 宽限期状态：`Normal`, `InGrace { entered_at, expires_at }`, `Expired` |
| `ChargeLayer` | 扣费层级：`IpfsPool` → `SubjectFunding` → `GracePeriod` |
| `ChargeResult<BlockNumber>` | 扣费结果：`Success { layer }`, `EnterGrace { expires_at }` |
| `UnpinReason` | Unpin 原因：`InsufficientFunds`, `ManualRequest`, `GovernanceDecision`, `OperatorOffline` |

### Config 参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `MaxCidHashLen` | `u32` | 最大 CID 哈希长度 |
| `MaxPeerIdLen` | `u32` | 最大 PeerId 长度 |
| `MinOperatorBond` | `Balance` | 运营者最低保证金（NEX 兜底值） |
| `MinOperatorBondUsd` | `u64` | 运营者最低保证金 USD 价值（精度 10^6） |
| `MinCapacityGiB` | `u32` | 最小存储容量（GiB） |
| `SubjectPalletId` | `PalletId` | 派生资金账户的 PalletId |
| `IpfsPoolAccount` | `AccountId` | IPFS 公共池账户 |
| `OperatorEscrowAccount` | `AccountId` | 运营者托管账户 |
| `MonthlyPublicFeeQuota` | `Balance` | 每月公共费用配额（默认 100 NEX） |
| `QuotaResetPeriod` | `BlockNumber` | 配额重置周期（默认 ~28天） |
| `DefaultBillingPeriod` | `u32` | 默认计费周期（默认 ~7天） |
| `OperatorGracePeriod` | `BlockNumber` | 运营者注销宽限期（默认 ~7天） |

### Extrinsics

#### 用户操作

| call_index | 函数 | 说明 |
|------------|------|------|
| 10 | `request_pin_for_subject` | 为 Subject 关联的 CID 发起 Pin 请求（四层扣费） |
| 32 | `request_unpin` | 用户主动取消固定 CID（按比例退款） |
| 21 | `fund_user_account` | 充值用户存储资金账户（混合方案） |
| 9 | `fund_subject_account` | 充值 Subject 资金账户（已弃用，向后兼容） |
| 34 | `cleanup_expired_cids` | 手动清理过期 CID 存储（任何人可调用，limit ≤ 50） |

#### 运营者操作

| call_index | 函数 | 说明 |
|------------|------|------|
| 3 | `join_operator` | 申请成为运营者（存入保证金，容量 ≥ MinCapacityGiB） |
| 4 | `update_operator` | 更新运营者元信息 |
| 5 | `leave_operator` | 退出运营者（无Pin立即退出，有Pin进入宽限期） |
| 7 | `report_probe` | OCW 自证在线（探测 /peers 含自身 peer_id） |
| 16 | `operator_claim_rewards` | 提取累计奖励 |
| 22 | `pause_operator` | 运营者自主暂停（status → Suspended） |
| 23 | `resume_operator` | 运营者恢复（status → Active） |

#### OCW Unsigned Extrinsics

| call_index | 函数 | 说明 |
|------------|------|------|
| 1 | `mark_pinned` | 上报 Pin 成功（signed） |
| 2 | `mark_pin_failed` | 上报 Pin 失败（signed） |
| 40 | `ocw_mark_pinned` | OCW 上报 Pin 成功（unsigned） |
| 41 | `ocw_mark_pin_failed` | OCW 上报 Pin 失败（unsigned） |
| 42 | `ocw_submit_assignments` | OCW 提交分层 Pin 分配（unsigned） |

#### 治理操作

| call_index | 函数 | 说明 |
|------------|------|------|
| 6 | `set_operator_status` | 设置运营者状态（0=Active, 1=Suspended, 2=Banned，校验 ≤ 2） |
| 8 | `slash_operator` | 扣罚运营者保证金 |
| 11 | `charge_due` | 手动触发到期扣费 |
| 12 | `set_billing_params` | 设置/暂停计费参数（部分更新） |
| 13 | `distribute_to_operators` | 分配运营者奖励 |
| 14 | `set_replicas_config` | 设置副本数配置（Level 0-3） |
| 15 | `update_tier_config` | 更新 Pin 分层配置（副本数 1-10，间隔 ≥ 600，费率 0.1x-10x） |
| 17 | `emergency_pause_billing` | 紧急暂停计费 |
| 18 | `resume_billing` | 恢复计费 |
| 19 | `set_storage_layer_config` | 设置分层存储策略配置 |
| 20 | `set_operator_layer` | 设置运营者层级（Core/Community） |
| 25 | `register_domain` | 手动注册业务域 |
| 26 | `update_domain_config` | 更新域配置 |
| 27 | `set_domain_priority` | 设置域巡检优先级（0-255） |
| 33 | `governance_force_unpin` | 强制下架 CID（违规内容） |

### 存储项

#### Pin 管理

| 存储 | 类型 | 说明 |
|------|------|------|
| `PendingPins` | `Map<Hash → (AccountId, u32, u64, u64, Balance)>` | 待处理 Pin 订单 |
| `PinMeta` | `Map<Hash → PinMetadata>` | Pin 元信息（副本数、大小、时间） |
| `PinStateOf` | `Map<Hash → u8>` | Pin 状态机（0=Requested, 1=Pinning, 2=Pinned, 3=Degraded, 4=Failed） |
| `PinAssignments` | `Map<Hash → BoundedVec<AccountId, 16>>` | 副本分配（运营者列表） |
| `PinSuccess` | `DoubleMap<Hash, AccountId → bool>` | 分配内成功标记 |
| `CidTier` | `Map<Hash → PinTier>` | CID 分层映射 |
| `PinTierConfig` | `Map<PinTier → TierConfig>` | 分层策略配置 |
| `CidToSubject` | `Map<Hash → BoundedVec<SubjectInfo, 8>>` | CID → Subject 反向映射 |
| `CidRegistry` | `Map<Hash → BoundedVec<u8, 128>>` | CID 明文注册表 |
| `CidLocks` | `Map<Hash → (reason, expiry)>` | CID 锁定记录（仲裁保护） |
| `CidUnpinReason` | `Map<Hash → UnpinReason>` | Unpin 原因记录 |

#### 运营者管理

| 存储 | 类型 | 说明 |
|------|------|------|
| `Operators` | `Map<AccountId → OperatorInfo>` | 运营者注册表 |
| `OperatorBond` | `Map<AccountId → Balance>` | 运营者保证金 |
| `OperatorSla` | `Map<AccountId → SlaStats>` | 运营者 SLA 统计 |
| `OperatorPinCount` | `Map<AccountId → u32>` | 运营者 Pin 数量索引（O(1)） |
| `OperatorUsedBytes` | `Map<AccountId → u64>` | 运营者实际存储字节数 |
| `OperatorPinStats` | `Map<AccountId → OperatorPinHealth>` | Pin 健康统计（含评分） |
| `OperatorRewards` | `Map<AccountId → Balance>` | 待提取奖励 |
| `ActiveOperatorIndex` | `Value<BoundedVec<AccountId, 256>>` | 活跃运营者索引（有界） |
| `PendingUnregistrations` | `Map<AccountId → BlockNumber>` | 待注销运营者（宽限期） |

#### 分层存储

| 存储 | 类型 | 说明 |
|------|------|------|
| `StorageLayerConfigs` | `Map<(SubjectType, PinTier) → StorageLayerConfig>` | 分层存储策略 |
| `LayeredPinAssignments` | `Map<Hash → LayeredPinAssignment>` | CID 分层分配记录 |
| `SimplePinAssignments` | `Map<Hash → BoundedVec<AccountId, 8>>` | 简化 Pin 分配 |
| `SimpleNodeStatsMap` | `Map<AccountId → SimpleNodeStats>` | 节点统计 |

#### 域管理

| 存储 | 类型 | 说明 |
|------|------|------|
| `RegisteredDomains` | `Map<BoundedVec → DomainConfig>` | 域注册表 |
| `DomainPins` | `DoubleMap<domain, Hash → ()>` | 域维度 Pin 索引 |
| `DomainPriority` | `Map<domain → u8>` | 域巡检优先级 |
| `DomainHealthStats` | `Map<domain → DomainStats>` | 域级健康统计 |

#### 计费系统

| 存储 | 类型 | 说明 |
|------|------|------|
| `BillingQueue` | `DoubleMap<BlockNumber, Hash → BillingTask>` | 周期扣费队列 |
| `PinBilling` | `Map<Hash → (BlockNumber, u128, u8)>` | CID 计费状态 |
| `PinSubjectOf` | `Map<Hash → (AccountId, u64)>` | CID 的 funding 来源 |
| `DueQueue` | `Map<BlockNumber → BoundedVec<Hash, 1024>>` | 到期队列 |
| `PricePerGiBWeek` | `Value<u128>` | 每 GiB·周单价（默认 1 NEX） |
| `BillingPeriodBlocks` | `Value<u32>` | 计费周期（默认 100,800 块 ≈ 7天） |
| `GraceBlocks` | `Value<u32>` | 宽限期（默认 5,184,000 块 ≈ 360天） |
| `BillingPaused` | `Value<bool>` | 计费暂停开关 |
| `PublicFeeQuotaUsage` | `Map<subject_id → (Balance, BlockNumber)>` | 公共配额使用记录 |
| `UserFundingBalance` | `Map<AccountId → Balance>` | 用户资金余额追踪 |
| `SubjectUsage` | `Map<(AccountId, u8, u64) → Balance>` | Subject 费用消耗 |
| `ExpiredCidQueue` | `Value<BoundedVec<Hash, 200>>` | 过期 CID 队列（O(1) 出队） |

#### 健康巡检

| 存储 | 类型 | 说明 |
|------|------|------|
| `HealthCheckQueue` | `DoubleMap<BlockNumber, Hash → HealthCheckTask>` | 巡检队列 |
| `HealthCheckStats` | `Value<GlobalHealthStats>` | 全局健康统计 |
| `OwnerPinIndex` | `Map<AccountId → BoundedVec<Hash, 1000>>` | 按所有者索引 CID |

### Runtime API

| 方法 | 说明 |
|------|------|
| `get_user_funding_account(user)` | 获取用户存储资金派生账户地址 |
| `get_user_funding_balance(user)` | 获取用户存储资金余额 |
| `get_subject_usage(user, domain, subject_id)` | 获取特定业务费用消耗 |
| `get_user_all_usage(user)` | 获取用户所有业务费用汇总 |

### WeightInfo 函数（27 个）

```
request_pin, mark_pinned, mark_pin_failed, charge_due(n),
set_billing_params, join_operator, update_operator, leave_operator,
set_operator_status, report_probe, slash_operator,
fund_subject_account, fund_user_account, set_replicas_config,
distribute_to_operators, set_storage_layer_config, set_operator_layer,
pause_operator, resume_operator, update_tier_config,
operator_claim_rewards, emergency_pause_billing, resume_billing,
register_domain, update_domain_config, request_unpin,
set_domain_priority, governance_force_unpin, cleanup_expired_cids(n)
```

---

## pallet-storage-lifecycle（生命周期管理）

### 概述

数据分级归档引擎，通过 `on_idle` 自动处理三级归档流水线：Active → L1 → L2 → Purge。支持多数据类型、差异化策略、清除保护、Active 延期与 L1 恢复。

### 公共 Trait 接口

| Trait | 说明 |
|-------|------|
| `ArchivableData` | 可归档数据接口，定义 `ArchivedL1`, `ArchivedL2`, `PermanentStats` 关联类型 + `can_archive_l1/l2`, `to_archived_l1`, `l1_to_l2`, `update_stats` 方法 |
| `StorageArchiver` | 存储归档器接口，由 pallet-storage-service 实现。提供 `scan_archivable`, `archive_records`, `scan_for_level`, `archive_to_level`, `restore_record`, `registered_data_types`, `query_archive_level` |
| `OnArchiveHandler` | 归档回调，归档完成后通知下游 pallet |

### 核心类型

| 类型 | 说明 |
|------|------|
| `ArchiveLevel` | 归档级别：`Active`(0), `ArchivedL1`(1), `ArchivedL2`(2), `Purged`(3) |
| `ArchiveConfig` | 全局归档配置（l1_delay, l2_delay, purge_delay, purge_enabled, max_batch_size） |
| `ArchivePolicy` | 按数据类型的归档策略（l1_delay, l2_delay, purge_delay, purge_enabled） |
| `ArchiveBatch` | 归档批次信息（batch_id, id_start, id_end, count, archived_at, level） |
| `ArchiveStatistics` | 归档统计（total_l1_archived, total_l2_archived, total_purged, total_bytes_saved） |

### Config 参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `L1ArchiveDelay` | `u32` | Active → L1 归档延迟（区块数） |
| `L2ArchiveDelay` | `u32` | L1 → L2 归档延迟（区块数） |
| `PurgeDelay` | `u32` | L2 → Purge 清除延迟（区块数） |
| `EnablePurge` | `bool` | 是否启用清除功能 |
| `MaxBatchSize` | `u32` | 每次 on_idle 最大处理数量 |
| `StorageArchiver` | trait | 存储归档器实现 |
| `OnArchive` | trait | 归档回调处理器 |
| `WeightInfo` | trait | 权重信息 |

### Extrinsics（全部 Root 权限）

| call_index | 函数 | 说明 |
|------------|------|------|
| 0 | `set_archive_config` | 设置全局归档配置（校验 l1 ≤ l2，purge 时 l2 ≤ purge） |
| 1 | `pause_archival` | 暂停自动归档 |
| 2 | `resume_archival` | 恢复自动归档 |
| 3 | `set_archive_policy` | 设置按数据类型的归档策略（覆盖全局配置） |
| 4 | `force_archive` | 强制归档指定数据（过滤后退/同级操作，清理延期和保护标志） |
| 5 | `protect_from_purge` | 设置清除保护（防止数据被 Purge） |
| 6 | `remove_purge_protection` | 移除清除保护 |
| 7 | `extend_active_period` | 延长数据 Active 期（extend_blocks ≥ 100） |
| 8 | `restore_from_archive` | 从 L1 归档恢复数据到 Active |

### 存储项

| 存储 | 类型 | 说明 |
|------|------|------|
| `ArchiveCursor` | `Map<data_type → u64>` | 归档游标（按数据类型） |
| `ArchiveBatches` | `Map<data_type → BoundedVec<ArchiveBatch, 100>>` | 归档批次记录 |
| `ArchiveStats` | `Map<data_type → ArchiveStatistics>` | 归档统计 |
| `ArchivalPaused` | `Value<bool>` | 归档暂停标志 |
| `ArchiveConfigOverride` | `Value<ArchiveConfig>` | 运行时可调归档配置（覆盖 genesis 常量） |
| `ArchivePolicies` | `Map<data_type → ArchivePolicy>` | 按类型归档策略 |
| `DataArchiveStatus` | `DoubleMap<data_type, data_id → ArchiveLevel>` | 数据归档状态跟踪 |
| `PurgeProtected` | `DoubleMap<data_type, data_id → bool>` | 清除保护标志 |
| `ActiveExtensions` | `DoubleMap<data_type, data_id → u64>` | Active 延期截止区块 |
| `TotalBatchCount` | `Map<data_type → u64>` | 累计批次计数 |

### Events

| 事件 | 说明 |
|------|------|
| `ArchivedToL1` | 数据已归档到 L1（data_type, count, saved_bytes） |
| `ArchivedToL2` | 数据已归档到 L2 |
| `DataPurged` | 数据已清除 |
| `ArchiveConfigUpdated` | 全局配置已更新 |
| `ArchivalPausedEvent` | 归档已暂停 |
| `ArchivalResumedEvent` | 归档已恢复 |
| `ArchivePolicySet` | 归档策略已设置 |
| `PurgeProtectionChanged` | 清除保护状态变更 |
| `DataForceArchived` | 数据强制归档 |
| `ActivePeriodExtended` | Active 期延长 |
| `DataRestored` | 数据已恢复 |
| `ArchivalWarning` | 归档前预警（数据达到 L1 延迟 80%） |
| `ArchivalBacklog` | 归档积压告警 |

### Errors

| 错误 | 说明 |
|------|------|
| `BatchQueueFull` | 归档批次已满（不应发生，LC3 修复后自动淘汰最旧批次） |
| `InvalidArchiveState` | 数据状态不允许归档 |
| `ArchivalAlreadyPaused` / `ArchivalNotPaused` | 暂停状态冲突 |
| `CannotRestoreFromLevel` | 无法从该级别恢复（仅支持 L1 → Active） |
| `InvalidConfig` | 配置参数无效（l1 > l2 等） |
| `ExtensionTooShort` | 延期太短（< 100 块） |
| `AlreadyProtected` / `NotProtected` | 保护状态冲突 |
| `RestoreFailed` | StorageArchiver 恢复失败 |

### on_idle 归档引擎

`on_idle` 自动执行三阶段流水线（带精确权重预算跟踪）：

```
┌─────────────────────────────────────────────────┐
│ 对每个注册数据类型循环：                          │
│                                                   │
│  阶段 1: Active → L1                              │
│    scan_for_level(L1) → 过滤已延期 → archive      │
│                                                   │
│  阶段 2: L1 → L2                                  │
│    scan_for_level(L2) → archive                   │
│                                                   │
│  阶段 3: L2 → Purge（如 purge_enabled）            │
│    scan_for_level(Purge) → 过滤受保护 → archive   │
│                                                   │
│  预警: 扫描接近 L1 阈值的数据 (80%)               │
│  积压: 检查待处理记录是否超过 3×batch_size         │
│                                                   │
│  ⚡ 超出 remaining_weight 预算则中止              │
└─────────────────────────────────────────────────┘
```

### Runtime API

| 方法 | 说明 |
|------|------|
| `get_archive_stats(data_type)` | 获取归档统计（l1, l2, purged, bytes_saved, last_archive_at） |
| `get_archive_config()` | 获取当前有效配置 |
| `get_data_status(data_type, data_id)` | 查询数据归档级别 |
| `is_archival_paused()` | 查询归档是否暂停 |

### WeightInfo 函数（9 个）

```
set_archive_config, pause_archival, resume_archival,
set_archive_policy, force_archive, protect_from_purge,
remove_purge_protection, extend_active_period, restore_from_archive
```

---

## 跨模块依赖

```
Evidence ──────────► StorageService (IpfsPinner / ContentRegistry)
Arbitration ───────► StorageService (CidLockManager) + StorageLifecycle
Trading/OTC ───────► StorageService (IpfsPinner) + StorageLifecycle
Trading/Swap ──────► StorageService (IpfsPinner) + StorageLifecycle
新业务 Pallet ─────► StorageService (ContentRegistry：一行代码接入)

StorageLifecycle ──► StorageService (StorageArchiver trait 实现)
StorageService ────► pallet-trading-common (DepositCalculator)
```

## 安全审计 (2026-02-23)

### pallet-storage-service 修复项

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| C1 | Critical | OCW storage writes are silent no-ops → unsigned tx (`ocw_mark_pinned`/`ocw_mark_pin_failed`/`ocw_submit_assignments`) | ✅ Fixed |
| C2 | Critical | `four_layer_charge` double-spend: withdraw burns tokens + operator claims again | ✅ Fixed |
| C3 | Critical | `charge_due` calls `distribute_to_pin_operators` again after `four_layer_charge` already did | ✅ Fixed |
| H1 | High | `count_operator_pins` O(N) full table scan → `OperatorPinCount` StorageMap O(1) + `ExpiredCidQueue` 替代 `PinBilling::iter()` | ✅ Fixed |
| H6 | Medium | 20+ extrinsics used hardcoded `weight(10_000)` → proper `T::WeightInfo::*()` calls (27 函数) | ✅ Fixed |
| M2 | Medium | `leave_operator` hardcoded 100,800 grace period → `OperatorGracePeriod` Config constant | ✅ Fixed |
| M3 | Medium | `set_operator_status` accepted any u8 → validate `status <= 2` | ✅ Fixed |
| M4 | Medium | `OperatorMetrics` missing `DecodeWithMemTracking` + `MaxEncodedLen` | ✅ Fixed |
| M5 | Medium | `emergency_pause_billing`/`resume_billing` unused `who` variable + hardcoded weight | ✅ Fixed |

### pallet-storage-lifecycle 修复项

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| LC1 | Medium | 4 types missing `DecodeWithMemTracking` | ✅ Fixed |
| LC2 | Low | `block_to_year_month` division by zero if `blocks_per_day == 0` | ✅ Fixed |
| LC3 | Medium | `ArchiveBatches` permanently fails when full (100 cap) → rotate oldest | ✅ Fixed |

### 验证

- `cargo check -p pallet-storage-service` ✅
- `cargo check -p pallet-storage-lifecycle` ✅
- `cargo check -p nexus-runtime` ✅
- `cargo test -p pallet-storage-service --lib` ✅ 13/13 pass
