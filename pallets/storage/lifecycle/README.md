# pallet-storage-lifecycle

存储生命周期管理模块，提供生产级分级归档系统。

## 模块概述

本模块为 Substrate 链上数据提供统一的生命周期管理机制，通过分级归档策略有效降低链上存储成本。支持三级存储层次，在 `on_idle` 中自动处理多阶段归档，并提供完整的治理控制和用户 API。

### 核心功能

| 编号 | 功能 | 描述 |
|------|------|------|
| **D1** | 三阶段归档 | `on_idle` 自动处理 Active→L1→L2→Purge |
| **D2** | 多级归档操作 | `StorageArchiver` trait 支持按级别扫描和归档 |
| **D3** | 归档回调 | `OnArchiveHandler` 通知下游 pallet |
| **G1** | 运行时可调配置 | `set_archive_config` 治理调参 |
| **G2** | 暂停/恢复归档 | `pause_archival` / `resume_archival` |
| **G3** | 按类型策略 | `set_archive_policy` 每种数据类型独立策略 |
| **G4** | 强制归档 + 清除保护 | `force_archive` / `protect_from_purge` |
| **U1** | 数据状态查询 | `query_data_status` 查询单条数据归档级别 |
| **U2** | 归档前预警 | `on_idle` 自动检测即将归档的数据并发出 `ArchivalWarning` 事件 |
| **U3** | Active 期延长 | `extend_active_period` 延长数据活跃期 |
| **U4** | 归档恢复 | `restore_from_archive` 从 L1 恢复 |
| **O1** | 批次统计 | 自动记录批次和归档统计 |
| **O2** | 仪表盘 API | Runtime API 和 `get_dashboard` 查询 |
| **O3** | 积压告警 | 待处理记录过多时发出告警事件 |
| **O4** | 基准权重 | `WeightInfo` trait 支持 |

### 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                    存储生命周期管理 (on_idle)                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌────────────┐  L1 delay  ┌────────────┐  L2 delay            │
│  │  Active    │ ─────────▶ │ ArchivedL1 │ ─────────▶           │
│  │  (完整)    │            │  (~50-80%) │                      │
│  └────────────┘            └────────────┘                      │
│       │                          │                              │
│   延期保护                   L2 delay                           │
│  (U3 extend)                     ▼                              │
│                            ┌────────────┐  Purge delay          │
│                            │ ArchivedL2 │ ─────────▶ Purged    │
│                            │  (~90%+)   │    (G4保护过滤)       │
│                            └────────────┘                       │
│                                                                 │
│  治理控制: G1(配置) G2(暂停) G3(策略) G4(保护)                   │
│  用户 API: U1(查询) U3(延期) U4(恢复)                            │
└─────────────────────────────────────────────────────────────────┘
```

### 归档级别

| 级别 | 值 | 存储节省 | 说明 |
|------|---|---------|------|
| **Active** | 0 | 0% | 完整存储 |
| **ArchivedL1** | 1 | 50-80% | 核心字段，压缩存储 |
| **ArchivedL2** | 2 | 90%+ | 仅统计摘要 |
| **Purged** | 3 | 100% | 完全删除，仅保留永久统计 |

## Extrinsics

| 调用 | 权限 | 描述 |
|------|------|------|
| `set_archive_config(config)` | Root | G1: 设置全局归档配置 |
| `pause_archival()` | Root | G2: 暂停归档 |
| `resume_archival()` | Root | G2: 恢复归档 |
| `set_archive_policy(data_type, policy)` | Root | G3: 设置按类型策略 |
| `force_archive(data_type, ids, level)` | Root | G4: 强制归档指定数据 |
| `protect_from_purge(data_type, data_id)` | Root | G4: 标记清除保护 |
| `remove_purge_protection(data_type, data_id)` | Root | G4: 移除清除保护 |
| `extend_active_period(data_type, data_id, blocks)` | Root | U3: 延长活跃期 |
| `restore_from_archive(data_type, data_id)` | Root | U4: 从 L1 恢复到 Active |

## 存储项

| 存储项 | 类型 | 描述 |
|--------|------|------|
| `ArchiveCursor` | `Map(DataType → u64)` | 归档游标 |
| `ArchiveBatches` | `Map(DataType → Vec<ArchiveBatch>)` | 最近100个批次记录 |
| `ArchiveStats` | `Map(DataType → ArchiveStatistics)` | 归档统计 |
| `ArchivalPaused` | `Value(bool)` | 归档暂停标志 (G2) |
| `ArchiveConfigOverride` | `Value(Option<ArchiveConfig>)` | 运行时配置覆盖 (G1) |
| `ArchivePolicies` | `Map(DataType → ArchivePolicy)` | 按类型策略 (G3) |
| `DataArchiveStatus` | `DoubleMap(DataType, u64 → ArchiveLevel)` | 数据归档状态 (U1) |
| `PurgeProtected` | `DoubleMap(DataType, u64 → bool)` | 清除保护标志 (G4) |
| `ActiveExtensions` | `DoubleMap(DataType, u64 → u64)` | 活跃期延长截止块 (U3) |
| `TotalBatchCount` | `Map(DataType → u64)` | 总批次计数 (O1) |

## 事件

| 事件 | 描述 |
|------|------|
| `ArchivedToL1 { data_type, count, saved_bytes }` | 数据归档到 L1 |
| `ArchivedToL2 { data_type, count, saved_bytes }` | 数据归档到 L2 |
| `DataPurged { data_type, count }` | 数据被清除 |
| `ArchiveConfigUpdated { config }` | 归档配置已更新 (G1) |
| `ArchivalPausedEvent` | 归档已暂停 (G2) |
| `ArchivalResumedEvent` | 归档已恢复 (G2) |
| `ArchivePolicySet { data_type, policy }` | 类型策略已设置 (G3) |
| `PurgeProtectionChanged { data_type, data_id, protected }` | 清除保护状态变更 (G4) |
| `DataForceArchived { data_type, data_ids, target_level }` | 数据被强制归档 (G4) |
| `ActivePeriodExtended { data_type, data_id, extended_until }` | 活跃期已延长 (U3) |
| `DataRestored { data_type, data_id, from_level }` | 数据已恢复 (U4) |
| `ArchivalWarning { data_type, approaching_count }` | 即将归档预警 (U2) |
| `ArchivalBacklog { data_type, pending_count }` | 积压告警 (O3) |

## 错误

| 错误 | 描述 |
|------|------|
| `BatchQueueFull` | 批次队列已满 |
| `InvalidArchiveState` | 当前状态不允许操作 |
| `ArchivalAlreadyPaused` | 归档已暂停 |
| `ArchivalNotPaused` | 归档未暂停 |
| `CannotRestoreFromLevel` | 仅 L1 可恢复 |
| `InvalidConfig` | 配置无效（延迟为0、purge_enabled时purge_delay为0等） |
| `ExtensionTooShort` | 延长期过短（<100块） |
| `AlreadyProtected` | 数据已受保护 |
| `NotProtected` | 数据未受保护 |
| `RestoreFailed` | 底层恢复操作失败 |

## 配置参数

| 参数 | 类型 | 描述 | 建议值 |
|------|------|------|--------|
| `L1ArchiveDelay` | `u32` | L1归档延迟（区块数） | ~7天（100,800块） |
| `L2ArchiveDelay` | `u32` | L2归档延迟 | ~30天（432,000块） |
| `PurgeDelay` | `u32` | 清除延迟 | ~90天（1,296,000块） |
| `EnablePurge` | `bool` | 是否启用清除 | `false` |
| `MaxBatchSize` | `u32` | 每次 on_idle 最大处理数 | `100` |
| `StorageArchiver` | `impl StorageArchiver` | 归档器实现 | - |
| `OnArchive` | `impl OnArchiveHandler` | 归档回调处理器 | - |
| `WeightInfo` | `impl WeightInfo` | 权重函数 | `SubstrateWeight` |

## 核心 Trait

### StorageArchiver

```rust
pub trait StorageArchiver {
    fn scan_archivable(delay: u64, max_count: u32) -> Vec<u64>;
    fn archive_records(ids: &[u64]);
    fn scan_for_level(data_type: &[u8], target: ArchiveLevel, delay: u64, max: u32) -> Vec<u64>;
    fn archive_to_level(data_type: &[u8], ids: &[u64], target: ArchiveLevel);
    fn registered_data_types() -> Vec<Vec<u8>>;
    fn query_archive_level(data_type: &[u8], data_id: u64) -> ArchiveLevel;
    fn restore_record(data_type: &[u8], data_id: u64, from: ArchiveLevel) -> bool;
}
```

### OnArchiveHandler

```rust
pub trait OnArchiveHandler {
    fn on_archived(data_type: &[u8], data_id: u64, from: ArchiveLevel, to: ArchiveLevel);
}
```

### ArchiveConfig / ArchivePolicy

```rust
pub struct ArchiveConfig {
    pub l1_delay: u32,
    pub l2_delay: u32,
    pub purge_delay: u32,
    pub purge_enabled: bool,
    pub max_batch_size: u32,
}

pub struct ArchivePolicy {
    pub l1_delay: u32,
    pub l2_delay: u32,
    pub purge_delay: u32,
    pub purge_enabled: bool,
}
```

## Runtime API

`StorageLifecycleApi` 提供以下查询接口（定义于 `runtime_api.rs`）：

| 方法 | 返回 | 描述 |
|------|------|------|
| `get_archive_stats(data_type)` | `(u64,u64,u64,u64,u64)` | L1/L2/Purged 数量、节省字节、最后归档时间 |
| `get_archive_config()` | `(u32,u32,u32,bool,u32)` | 当前有效配置 |
| `get_data_status(data_type, data_id)` | `u8` | 数据归档级别 |
| `is_archival_paused()` | `bool` | 归档是否暂停 |

## 配置示例

```rust
parameter_types! {
    pub const L1ArchiveDelay: u32 = 100_800;  // ~7天
    pub const L2ArchiveDelay: u32 = 432_000;  // ~30天
    pub const PurgeDelay: u32 = 1_296_000;    // ~90天
    pub const EnablePurge: bool = false;
    pub const MaxBatchSize: u32 = 100;
}

impl pallet_storage_lifecycle::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type L1ArchiveDelay = L1ArchiveDelay;
    type L2ArchiveDelay = L2ArchiveDelay;
    type PurgeDelay = PurgeDelay;
    type EnablePurge = EnablePurge;
    type MaxBatchSize = MaxBatchSize;
    type StorageArchiver = MyArchiver;
    type OnArchive = MyArchiveHandler;
    type WeightInfo = pallet_storage_lifecycle::SubstrateWeight;
}
```

## 测试覆盖

76 个测试覆盖所有功能：

- 基础工具函数（`block_to_year_month`, `amount_to_tier`, `ArchiveLevel` 转换）
- 批次记录与统计（record_batch, cursor, bytes_saved, queue eviction）
- D1: 三阶段归档（Active→L1→L2→Purge, 权重不足跳过）
- D3: 归档回调验证
- U2: 归档前预警（on_idle 事件 + query_approaching_archival 查询）
- G1: 配置管理（设置、无效拒绝、Root 权限、常量回退）
- G2: 暂停/恢复（on_idle 跳过、重复暂停拒绝）
- G3: 按类型策略（设置、无效拒绝、全局回退）
- G4: 强制归档 + 清除保护（保护/移除/on_idle 过滤）
- U1: 数据状态查询
- U3: Active 延长（叠加、过短拒绝、非活跃拒绝、on_idle 过滤）
- U4: 归档恢复（L1恢复、非L1拒绝、底层失败）
- O1/O2: 统计和仪表盘
- 综合流程（全生命周期、策略覆盖）
- H1-R1: on_idle 权重准确性（扫描开销、项目开销）
- H2-R1: force_archive 回调（回调触发、from_level 正确性）
- M1-R1: purge_delay 校验（启用时拒绝零值、禁用时允许）
- M2-R1: TotalBatchCount 递增（手动和 on_idle）
- H1-R2: on_idle 权重不超过 remaining_weight（普通和紧缩限制）
- M1-R2: on_idle 回调读取实际 from_level
- M2-R2: 积压告警事件可触发
- M3-R2: force_archive 清理 ActiveExtensions 和 PurgeProtected
- H1-R3: force_archive 跳过后退/同级转换（3 个测试）
- M1-R3: on_idle 中途超预算中止
- M2-R3: 延迟顺序校验（config 和 policy 各 3 个测试）
- M1-R4: restore_from_archive 触发 OnArchive 回调（2 个测试）
- M2-R4: DataForceArchived 事件仅含实际归档 ID（2 个测试）
- M3-R4: force_archive 缓存 from_level 正确性

## 版本历史

| 版本 | 变更 |
|------|------|
| v0.1.0 | 初始框架：ArchivableData trait, on_idle 基础归档 |
| v0.2.0 | 生产级实现：多级 StorageArchiver, OnArchiveHandler, 9 个治理/用户 extrinsics, Runtime API, 45 个测试 |
| v0.2.1 | 深度审计 Round 1：H1(权重准确性), H2(force_archive回调), M1(purge_delay校验), M2(TotalBatchCount), M3(死代码), L1(死错误), L2(README同步) — 55 个测试 |
| v0.2.2 | 深度审计 Round 2：H1(权重不超过remaining_weight), M1(回调读取实际from_level), M2(积压告警回归), M3(force_archive存储清理), L1(死BatchCompleted), L2(死ArchiveRecord), L4(Cargo版本) — 61 个测试 |
| v0.2.3 | 深度审计 Round 3：H1(force_archive拒绝后退转换), M1(on_idle中途预算中止), M2(延迟顺序校验l1≤l2≤purge), L1(移除死new()) — 71 个测试 |
| v0.2.4 | 深度审计 Round 4：M1(restore_from_archive触发OnArchive回调), M2(DataForceArchived事件仅含forward_ids), M3(force_archive缓存from_level避免二次读取) — 76 个测试 |

## 许可证

MIT License

## 作者

Nexus Team
