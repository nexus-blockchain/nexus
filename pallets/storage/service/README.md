# pallet-storage-service

> 路径：`pallets/storage/service/`

IPFS 存储服务核心模块，提供去中心化内容固定（Pin）、运营者管理、分层存储策略、自动计费和 OCW 健康巡检功能。

## 设计理念

- **多副本冗余**：多运营者节点分层存储，确保高可用性
- **分层存储**：Critical / Standard / Temporary 三级策略，按数据重要性差异化配置
- **自动化**：OCW 健康巡检、故障自动修复、周期扣费、过期清理
- **经济激励**：保证金锁定 + SLA 统计 + 奖励分配 + 阶梯惩罚
- **域管理**：支持业务域自动 PIN 注册，按域统计和优先级调度

## 核心功能

### 1. Pin 管理
| 功能 | 说明 |
|------|------|
| 多层级请求 | Critical / Standard / Temporary 三级分层 |
| 自动分配 | 基于 Layer（Core/Community）、健康度、容量选择运营者 |
| 多副本冗余 | 按层级配置副本数（5/3/1），不足时自动补充 |
| 状态追踪 | Requested(0) → Pinning(1) → Pinned(2) → MarkedForUnpin |
| 续期/升级 | 用户可续期延长计费、升级 Tier 提高副本数 |
| 批量操作 | 支持批量取消 Pin（最多20个/次） |
| CID 锁定 | 仲裁期间锁定 CID 防止删除（治理可覆盖） |

### 2. 运营者管理
| 功能 | 说明 |
|------|------|
| 三层分类 | Core(Layer 1) / Community(Layer 2) / External(Layer 3, 预留) |
| 保证金 | USD 动态计算（`DepositCalculator`），支持追加/减少 |
| 状态管理 | Active(0) / Suspended(1) / Banned(2) |
| SLA 统计 | 成功率、健康度评分(0-100)、容量使用率 |
| 自动暂停 | 健康分 < 30 自动 Suspended |
| 注销宽限 | 有 Pin 时进入7天宽限期，OCW 迁移后退还保证金 |
| Pin 迁移 | 治理可将运营者的 Pin 迁移到其他运营者 |

### 3. 分层策略（PinTier）
| 层级 | 副本数 | 巡检周期 | 费率系数 | 宽限期 | 适用场景 |
|------|-------|---------|---------|-------|---------|
| Critical | 5 | 6小时(7200块) | 1.5x | 7天 | 证据、法律文件 |
| Standard | 3 | 24小时(28800块) | 1.0x | 7天 | 一般业务数据 |
| Temporary | 1 | 7天(604800块) | 0.5x | 3天 | 临时数据、缓存 |

### 4. 分层存储策略（StorageLayerConfig）
按 `SubjectType × PinTier` 细粒度配置 Layer 1/2 副本分配：

| 数据类型 | Core副本 | Community副本 | 允许External | 最低副本 |
|---------|---------|-------------|-------------|---------|
| Evidence | 5 | 0 | ✗ | 3 |
| Product | 2 | 1 | ✗ | 1 |
| Entity | 2 | 1 | ✗ | 1 |
| Shop | 2 | 1 | ✗ | 1 |
| General | 2 | 1 | ✗ | 1 |

### 5. 计费机制
- **五层扣费顺序**：用户级配额 → Entity 国库 → UserFunding → IpfsPool 兜底 → 宽限期
- **扣费周期**：默认每周（100,800 块 ≈ 7天）
- **计费精度**：按 MiB 向上取整（PricePerGiBWeek / 1024），对小文件更公平
- **宽限期**：按 Tier 配置（Critical/Standard = 7天，Temporary = 3天），指数退避重试
- **续期**：用户可预付1-52个周期续费
- **退款**：提前取消 Pin 按 BillingQueue 剩余预付时间比例退款
- **资金安全**：Pool 分配给运营者的奖励使用 `reserve` 锁定，防止超支
- **运营者激励**：按健康度评分加权分配（非等额），激励高质量服务

### 6. 域管理（Domain）
- **SubjectType 自动域派生**：`IpfsPinner` 调用方传入的 `SubjectType` 自动映射为域名，CID 按业务类型分类
- **自动注册**：业务 pallet 通过 `ContentRegistry` 自动注册域
- **治理管理**：手动注册域、更新配置、设置优先级
- **域统计**：按域统计 Pin 数量、存储容量、健康状态
- **优先级调度**：OCW 按域优先级顺序巡检（evidence=0, otc=10, general=20）

#### 域名映射规则（SubjectType → DomainPins 域名）
| SubjectType | 域名 | 典型调用方 |
|-------------|------|-----------|
| Evidence | `"evidence"` | pallet-evidence |
| Product | `"product"` | pallet-entity-product |
| Entity | `"entity"` | pallet-entity-registry |
| Shop | `"shop"` | pallet-entity-shop |
| General | `"general"` | 用户直接调用 `request_pin_for_subject` |
| Custom(name) | name 内容 | `ContentRegistry::register_content` 自定义域 |

### 7. on_finalize 自动化任务
每个区块结束时按优先级执行：

| 任务 | 限流 | 说明 |
|------|------|------|
| 周期扣费 | 20/块 | cursor 分页扫描 BillingQueue，五层回退扣费 |
| 健康巡检 | 10/块 | cursor 分页扫描 HealthCheckQueue，自动修复降级副本；幽灵条目（PinMeta 已删除）自动丢弃 |
| 运营者注销 | 5/块 | 检查宽限期到期，无 Pin 则退还保证金 |
| 过期CID清理 | 5/块 | 从 ExpiredCidQueue 出队清理关联存储（含 CidEntityOf、PendingPins，O(1)） |
| 域统计更新 | 每24h | 按域汇总 Pin 健康状态，更新全局统计 |

### 7.1 on_idle 孤儿 CID 扫描
利用剩余区块权重，分页扫描 `PinMeta`，检测并自动回收孤儿 CID：

| 条件 | 判定 |
|------|------|
| `PinMeta` 存在但 `PinSubjectOf` 缺失 | 无属主的孤儿 CID |
| `PinBilling` state=2 | 已在清理队列中，跳过 |

- **每块最多处理 10 条**，使用 `OrphanSweepCursor` 跨块分页
- 发现孤儿后调用 `mark_cid_for_unpin`，发射 `OrphanCidDetected` 事件
- 全表扫完后游标自动重置，开始下一轮

### 7.2 级联清理路径
Entity 关闭/封禁时的完整清理链：

```
Entity close/ban
  ├── unpin_all_entity_cids (logo_cid, description_cid, contact_cid, metadata_uri)
  ├── force_close_shop(shop_id) for each shop
  │     ├── ipfs_unpin_all_shop_cids (logo/description/address/business_hours/policies)
  │     └── ProductProvider::force_unpin_shop_products(shop_id)
  │           └── unpin per product: name/images/detail/tags/sku CID
  └── EntityShops::remove
```

### 8. OCW（Offchain Worker）
- **Pin 管理**：扫描 PendingPins，调用本地 IPFS API (`http://127.0.0.1:5001/api/v0/pin/add`) 执行 Pin
- **健康巡检**：GET `/api/v0/pin/ls?arg=<CID>` 检查副本状态，自动补充不足副本
- **状态上报**：通过 unsigned extrinsic 提交分配/Pin成功/失败/健康状态
- **物理删除**：调用 IPFS API 对过期 CID 执行 unpin
- **节点身份**：从 OCW 本地存储 `/memo/ipfs/node_account` 读取节点账户
- **CID 解析**：从 `/memo/ipfs/cid/<hash_hex>` 读取 CID 明文（不存在则降级为 `<redacted>`）
- **智能分配**：`optimized_pin_allocation` 基于容量和健康度评分选择最优节点

## 主要类型

### SubjectType（业务域）
```rust
pub enum SubjectType {
    Evidence,           // 证据（最高优先级，domain=0）
    Product,            // 商品元数据（domain=10）
    Entity,             // 实体元数据（domain=11）
    Shop,               // 店铺元数据（domain=12）
    General,            // 通用存储（domain=98）
    Custom(BoundedVec), // 自定义域（domain=99）
}
```

### OperatorLayer（运营者分层）
```rust
pub enum OperatorLayer {
    Core,       // Layer 1 - 项目方运营，最高优先级
    Community,  // Layer 2 - 社区运营
    External,   // Layer 3 - 外部网络（预留）
}
```

### PinTier（分层等级）
```rust
pub enum PinTier {
    Critical,   // 关键级：5副本，6小时巡检
    Standard,   // 标准级：3副本，24小时巡检（默认）
    Temporary,  // 临时级：1副本，7天巡检
}
```

### UnpinReason（Unpin 原因）
```rust
pub enum UnpinReason {
    InsufficientFunds,   // 宽限期过期
    ManualRequest,       // 用户主动取消
    GovernanceDecision,  // 治理强制下架
    OperatorOffline,     // 运营者长期离线
}
```

### OperatorInfo（运营者信息结构体）
```rust
pub struct OperatorInfo<T: Config> {
    pub peer_id: BoundedVec<u8, T::MaxPeerIdLen>,  // IPFS PeerId
    pub capacity_gib: u32,                          // 声明存储容量（GiB）
    pub endpoint_hash: T::Hash,                     // IPFS Cluster API 端点哈希
    pub cert_fingerprint: Option<T::Hash>,          // TLS 证书指纹（可选）
    pub status: u8,                                 // 0=Active, 1=Suspended, 2=Banned
    pub registered_at: BlockNumber,                 // 注册时间（区块高度）
    pub layer: OperatorLayer,                       // Core/Community/External 分层
    pub priority: u8,                               // 优先级 0-255（越小越优先）
}
```

### PinMetadata（Pin 元信息）
```rust
pub struct PinMetadata<BlockNumber> {
    pub replicas: u32,              // 副本数
    pub size: u64,                  // 文件大小（字节）
    pub created_at: BlockNumber,    // 创建时间
    pub last_activity: BlockNumber, // 最后活动时间
}
```

### SlaStats（运营者 SLA 统计）
```rust
pub struct SlaStats<T: Config> {
    pub pinned_bytes: u64,       // 已固定字节数
    pub probe_ok: u32,           // 探测成功次数
    pub probe_fail: u32,         // 探测失败次数
    pub degraded: u32,           // 降级次数
    pub last_update: BlockNumber,
}
```

### 其他核心类型（定义于 types.rs）
| 类型 | 说明 |
|------|------|
| `SubjectInfo` | CID 关联的 Subject 信息（type, id, funding_share） |
| `DomainConfig` | 域配置（auto_pin, default_tier, subject_type_id, owner_pallet） |
| `TierConfig` | 分层配置（replicas, health_check_interval, fee_multiplier, grace_period） |
| `HealthCheckTask` | 健康巡检任务（cid_hash, tier, next_check, failures） |
| `HealthStatus` | 健康状态枚举（Healthy/Degraded/Critical/Unknown） |
| `GlobalHealthStats` | 全局健康统计（total_pins, healthy/degraded/critical count） |
| `DomainStats` | 域级统计信息 |
| `BillingTask` | 扣费任务（cid_hash, amount_per_period, grace_status） |
| `GraceStatus` | 宽限状态（Normal/InGrace/Expired） |
| `ChargeLayer` | 扣费层级（IpfsPool/SubjectFunding/EntityTreasury） |
| `ChargeResult` | 扣费结果（Success/EnterGrace） |
| `StorageLayerConfig` | 分层存储策略配置（core/community replicas, min_total） |
| `LayeredOperatorSelection` | 分层运营者选择结果 |
| `LayeredPinAssignment` | CID 分层分配记录 |
| `OperatorPinHealth` | 运营者 Pin 健康统计（total/healthy/failed, health_score） |
| `OperatorMetrics` | 运营者综合指标（供 RPC 聚合返回） |
| `SimpleNodeStats` | 简化节点统计（PIN 分配评分用） |
| `SimplePinStatus` | 简化 PIN 状态（Pending/Pinned/Failed/Restored） |

## 存储项

### Pin 管理
| 存储 | 说明 |
|------|------|
| `PendingPins` | Map(Hash → (payer, replicas, subject_id, size, deposit)) |
| `PinMeta` | Map(Hash → PinMetadata) — 副本数、大小、时间 |
| `PinStateOf` | Map(Hash → u8) — 0=Requested, 1=Pinning, 2=Pinned, 3=Degraded, 4=Failed |
| `PinAssignments` | Map(Hash → BoundedVec\<AccountId, 16\>) — 副本运营者 |
| `PinSuccess` | DoubleMap(Hash, AccountId → bool) — 成功标记 |
| `PinSubjectOf` | Map(Hash → (AccountId, u64)) — CID→owner 映射 |
| `CidToSubject` | Map(Hash → BoundedVec\<SubjectInfo, 8\>) — CID→Subject |
| `CidTier` | Map(Hash → PinTier) — CID 分层等级 |
| `CidRegistry` | Map(Hash → BoundedVec\<u8, 128\>) — CID 明文 |
| `OwnerPinIndex` | Map(AccountId → BoundedVec\<Hash, 1000\>) — 用户 CID 索引 |
| `CidLocks` | Map(Hash → (reason, Option\<BlockNumber\>)) — 仲裁锁 |
| `CidUnpinReason` | Map(Hash → UnpinReason) |

### 运营者管理
| 存储 | 说明 |
|------|------|
| `Operators` | Map(AccountId → OperatorInfo) |
| `OperatorBond` | Map(AccountId → Balance) — 保证金 |
| `OperatorUsedBytes` | Map(AccountId → u64) — 实际存储字节数 |
| `OperatorSla` | Map(AccountId → SlaStats) |
| `OperatorPinStats` | Map(AccountId → OperatorPinHealth) — 健康统计 |
| `OperatorPinCount` | Map(AccountId → u32) — Pin 数量索引 O(1) |
| `OperatorRewards` | Map(AccountId → Balance) — 待提取奖励 |
| `ActiveOperatorIndex` | Value(BoundedVec\<AccountId, 256\>) — 活跃索引 |
| `PendingUnregistrations` | Map(AccountId → BlockNumber) — 注销宽限期 |

### 计费系统
| 存储 | 说明 |
|------|------|
| `PricePerGiBWeek` | Value(u128) — 每 GiB·周单价（默认 1e9） |
| `BillingPeriodBlocks` | Value(u32) — 扣费周期（默认 100,800） |
| `GraceBlocks` | Value(u32) — 宽限期（按Tier: 201,600/86,400） |
| `MaxChargePerBlock` | Value(u32) — 每块最大扣费数（默认 50） |
| `BillingPaused` | Value(bool) — 计费暂停开关 |
| `PinBilling` | Map(Hash → (BlockNumber, u128, u8)) — CID 计费状态 |
| `BillingQueue` | DoubleMap(BlockNumber, Hash → BillingTask) |
| `BillingSettleCursor` | Value(BlockNumber) — 计费游标 |
| `DueQueue` | Map(BlockNumber → BoundedVec\<Hash, 1024\>) |
| `DueEnqueueSpread` | Value(u32) — 扩散宽度（默认 10） |
| `ExpiredCidPending` | Value(bool) — 有无待清理过期 CID |
| `ExpiredCidQueue` | Value(BoundedVec\<Hash, 200\>) |
| `OrphanSweepCursor` | Value(BoundedVec\<u8, 128\>) — 孤儿扫描分页游标 |

### 配额与资金
| 存储 | 说明 |
|------|------|
| `PublicFeeQuotaUsage` | Map(AccountId → (used, reset_block))（用户级配额） |
| `CidBillingDueBlock` | Map(Hash → BlockNumber) — CID计费反向索引 |
| `TotalChargedFromPool` | Value(Balance) — 累计池扣款 |
| `TotalChargedFromSubject` | Value(Balance) — 累计用户扣款 |
| `UserFundingBalance` | Map(AccountId → Balance) — 用户资金余额 |
| `SubjectUsage` | Map((AccountId, domain, subject_id) → Balance) |

### 副本数、分层、域、健康
| 存储 | 说明 |
|------|------|
| `ReplicasForLevel0~3` | Value(u32) — 各级别推荐副本数（2/3/5/7） |
| `MinReplicasThreshold` | Value(u32) — 最小副本数（默认 2） |
| `PinTierConfig` | Map(PinTier → TierConfig) |
| `StorageLayerConfigs` | Map((SubjectType, PinTier) → StorageLayerConfig) |
| `LayeredPinAssignments` | Map(Hash → LayeredPinAssignment) |
| `DomainPins` | DoubleMap(domain, Hash → ()) — 域 Pin 索引 |
| `RegisteredDomains` | Map(domain → DomainConfig) |
| `DomainHealthStats` | Map(domain → DomainStats) |
| `DomainPriority` | Map(domain → u8) — 优先级（0=最高） |
| `HealthCheckQueue` | DoubleMap(BlockNumber, Hash → HealthCheckTask) |
| `HealthCheckStats` | Value(GlobalHealthStats) |
| `HealthCheckSettleCursor` | Value(BlockNumber) |
| `SimpleNodeStatsMap` | Map(AccountId → SimpleNodeStats) |
| `SimplePinAssignments` | Map(Hash → BoundedVec\<AccountId, 8\>) |
| `PricingParams` | Value(BoundedVec\<u8, 8192\>) |
| `SubjectMinReserve` | Value(Balance) |

## Extrinsics

### 用户接口
| 方法 | call_index | 权限 | 说明 |
|------|-----------|------|------|
| `request_pin_for_subject` | 10 | Signed | 为 Subject 固定 CID，五层扣费 |
| `fund_user_account` | 21 | Signed | 为用户资金账户充值 |
| `fund_subject_account` | 9 | Signed | ⚠️ 已废弃，用 `fund_user_account` |
| `request_unpin` | 32 | Signed(Owner) | 取消固定 CID（按比例退款） |
| `batch_unpin` | 48 | Signed(Owner) | 批量取消 Pin（≤20个） |
| `renew_pin` | 45 | Signed(Owner) | 续期 Pin（1-52个周期） |
| `upgrade_pin_tier` | 46 | Signed(Owner) | 升级 Pin 分层等级 |
| `fund_ipfs_pool` | 44 | Signed | 向 IPFS 公共池充值 |

### 运营者接口
| 方法 | call_index | 权限 | 说明 |
|------|-----------|------|------|
| `join_operator` | 3 | Signed | 注册运营者，锁定保证金 |
| `update_operator` | 4 | Signed(Operator) | 更新运营者元信息 |
| `leave_operator` | 5 | Signed(Operator) | 注销运营者（宽限期机制） |
| `pause_operator` | 22 | Signed(Operator) | 暂停接单（Active→Suspended） |
| `resume_operator` | 23 | Signed(Operator) | 恢复接单（Suspended→Active） |
| `report_probe` | 7 | Signed(Operator) | OCW 上报心跳 |
| `operator_claim_rewards` | 16 | Signed(Operator) | 领取奖励 |
| `top_up_bond` | 49 | Signed(Operator) | 追加保证金 |
| `reduce_bond` | 50 | Signed(Operator) | 减少保证金（≥最低要求） |

### OCW 签名接口
| 方法 | call_index | 权限 | 说明 |
|------|-----------|------|------|
| `mark_pinned` | 1 | Signed(OCW) | 上报 Pin 成功 |
| `mark_pin_failed` | 2 | Signed(OCW) | 上报 Pin 失败 |

### OCW Unsigned 接口
| 方法 | call_index | 说明 |
|------|-----------|------|
| `ocw_mark_pinned` | 40 | OCW 上报 Pin 成功（unsigned） |
| `ocw_mark_pin_failed` | 41 | OCW 上报 Pin 失败（unsigned） |
| `ocw_submit_assignments` | 42 | OCW 提交分层 Pin 分配（unsigned） |
| `ocw_report_health` | 43 | OCW 上报健康巡检结果（unsigned） |

### 公共清理接口
| 方法 | call_index | 权限 | 说明 |
|------|-----------|------|------|
| `cleanup_expired_cids` | 34 | Signed(Any) | 清理过期 CID 存储（≤50个） |
| `cleanup_expired_locks` | 51 | Signed(Any) | 清理过期 CID 锁（≤20个） |

### 治理接口
| 方法 | call_index | 说明 |
|------|-----------|------|
| `set_operator_status` | 6 | 设置运营者状态（0/1/2） |
| `slash_operator` | 8 | 惩罚运营者保证金 |
| `charge_due` | 11 | 手动处理到期扣费 |
| `set_billing_params` | 12 | 设置计费参数（部分更新） |
| `distribute_to_operators` | 13 | 分配收益给运营者 |
| `set_replicas_config` | 14 | 设置副本数配置 |
| `update_tier_config` | 15 | 更新分层配置（副本/巡检/费率） |
| `emergency_pause_billing` | 17 | 紧急暂停自动扣费 |
| `resume_billing` | 18 | 恢复自动扣费 |
| `set_storage_layer_config` | 19 | 设置分层存储策略 |
| `set_operator_layer` | 20 | 设置运营者层级 |
| `register_domain` | 25 | 手动注册业务域 |
| `update_domain_config` | 26 | 更新域配置 |
| `set_domain_priority` | 27 | 设置域巡检优先级 |
| `governance_force_unpin` | 33 | 强制下架 CID |
| `migrate_operator_pins` | 47 | 迁移运营者 Pin（≤100个） |

## Trait 接口

### SubjectOwnerProvider（Subject 所有者查询）
```rust
pub trait SubjectOwnerProvider<AccountId> {
    fn owner_of(subject_id: u64) -> Option<AccountId>;
}
```

### StoragePin（统一存储 Pin 接口，供所有业务 pallet 调用）
```rust
pub trait StoragePin<AccountId> {
    fn pin(
        owner: AccountId,
        domain: &[u8],       // "evidence", "product", "entity", "shop" 等
        subject_id: u64,
        entity_id: Option<u64>, // 所属 Entity（用于 Entity 国库扣费层）
        cid: Vec<u8>,
        tier: PinTier,       // Critical / Standard / Temporary
    ) -> DispatchResult;

    fn unpin(owner: AccountId, cid: Vec<u8>) -> DispatchResult;
}
```

> `entity_id` 由业务 pallet 在 pin 时提供（entity/shop/product 传 `Some(eid)`，evidence 传 `None`），
> 存入 `CidEntityOf` 存储，计费时用于 Entity 国库扣费层。
>
> 合并了原 `IpfsPinner`（6 参数）和 `ContentRegistry`（4 参数）两个 trait。
> `domain` 字符串自动映射为 `SubjectType` 和 `DomainPins` 域索引，
> `size_bytes` 由实现方内部估算（`cid.len() * 1024`），调用方无需关心。

### CidLockManager（仲裁锁定 CID）
```rust
pub trait CidLockManager<Hash, BlockNumber> {
    fn lock_cid(cid_hash: Hash, reason: Vec<u8>, until: Option<BlockNumber>) -> DispatchResult;
    fn unlock_cid(cid_hash: Hash, reason: Vec<u8>) -> DispatchResult;
    fn is_locked(cid_hash: &Hash) -> bool;
}
```

## 配置参数（Config）

### 常量
| 参数 | 说明 | 默认值 |
|------|------|-------|
| `MinOperatorBond` | 最小保证金（NEX 兜底值） | 100 UNIT |
| `MinOperatorBondUsd` | 最小保证金（USD，精度10^6） | 100 USDT |
| `MinCapacityGiB` | 运营者最小容量 | 10 GiB |
| `MaxCidHashLen` | CID 哈希最大长度 | 8192 |
| `MaxPeerIdLen` | PeerId 最大长度 | 32 |
| `SubjectPalletId` | 派生子账户 PalletId | — |
| `MonthlyPublicFeeQuota` | 每月公共配额 | 10 NEX |
| `QuotaResetPeriod` | 配额重置周期 | 432,000 块(~30天) |
| `DefaultBillingPeriod` | 默认扣费周期 | 432,000 块(~30天) |
| `OperatorGracePeriod` | 运营者注销宽限期 | 100,800 块(~7天) |

### 外部类型
| 参数 | 说明 |
|------|------|
| `Currency` | 货币接口（Currency + ReservableCurrency） |
| `Balance` | 余额类型（AtLeast32BitUnsigned） |
| `GovernanceOrigin` | 治理 Origin（配置/惩罚/管理） |
| `DepositCalculator` | 保证金动态计算器（USD → NEX） |
| `FeeCollector` | 费用接收账户接口 |
| `EntityFunding` | Entity 国库扣费接口（由 entity-registry 实现，无需时配 `()`） |
| `WeightInfo` | 权重信息 |

### 运行时存储参数
| 参数 | 说明 | 默认值 |
|------|------|-------|
| `PricePerGiBWeek` | 每 GiB 周单价 | 1e9 |
| `BillingPeriodBlocks` | 扣费周期 | 100,800 (~7天) |
| `GraceBlocks` | 宽限期（按Tier覆盖） | Critical/Standard=201,600(~7天), Temporary=86,400(~3天) |
| `MaxChargePerBlock` | 每块最大扣费数 | 50 |
| `DueEnqueueSpread` | 到期队列扩散宽度 | 10 |

### 关键账户
| 参数 | 说明 |
|------|------|
| `IpfsPoolAccount` | IPFS 公共池（pallet-storage-treasury 补充） |
| `OperatorEscrowAccount` | 运营者托管账户（服务费接收方） |
| `FeeCollector` | 费用接收账户（Treasury 或平台） |

## 错误码

### 基础错误
| 错误 | 说明 |
|------|------|
| `BadParams` | 参数非法 |
| `OrderNotFound` | CID/订单不存在 |
| `BadStatus` | 无效状态 |
| `NotOwner` | 非所有者（无权限操作） |

### 运营者错误
| 错误 | 说明 |
|------|------|
| `OperatorNotFound` | 运营者不存在 |
| `OperatorExists` | 运营者已存在 |
| `OperatorBanned` | 运营者已被禁用 |
| `AlreadyPaused` | 运营者已暂停（无法再次暂停） |
| `NotPaused` | 运营者未暂停（无法恢复） |
| `InsufficientBond` | 保证金不足 |
| `InsufficientCapacity` | 容量不足 |
| `HasActiveAssignments` | 仍存在未完成的副本分配，禁止退出 |
| `OperatorNotAssigned` | 调用方未被指派到该 CID 的副本分配 |
| `AssignmentNotFound` | 分配不存在 |
| `NoActiveOperators` | 没有活跃的运营者（无法分配奖励） |
| `InsufficientEscrowBalance` | 运营者托管账户余额不足 |
| `NotEnoughOperators` | 可用运营者不足（活跃数 < 副本数） |
| `NoAvailableOperators` | 没有可用的 IPFS 运营者 |
| `NoRewardsAvailable` | 无可用奖励（余额为零） |
| `NoOperatorsAssigned` | 没有分配运营者（Pin 未成功） |

### Pin 错误
| 错误 | 说明 |
|------|------|
| `CidAlreadyPinned` | CID 已被 Pin，禁止重复 |
| `AlreadyPinned` | 已经 Pin 过（避免重复） |
| `WeightOverflow` | 计算权重时发生溢出 |

### 计费错误
| 错误 | 说明 |
|------|------|
| `SubjectFundingInsufficientBalance` | SubjectFunding 账户余额不足 |
| `GraceExpired` | 宽限期已过（无法再扣费） |

### 分层/配置错误
| 错误 | 说明 |
|------|------|
| `TierConfigNotFound` | 分层配置未找到 |
| `InvalidReplicas` | 副本数无效（必须 1-10） |
| `IntervalTooShort` | 巡检间隔太短（必须 ≥ 600 块） |
| `InvalidMultiplier` | 费率系数无效（必须 0.1x-10x） |
| `HealthCheckTaskNotFound` | 健康巡检任务未找到 |
| `BillingTaskNotFound` | 扣费任务未找到 |
| `InsufficientNodes` | 节点数量不足 |
| `TooManyNodes` | 节点数量超过 BoundedVec 限制 |

### 域管理错误
| 错误 | 说明 |
|------|------|
| `DomainTooLong` | 域名超过 32 字节 |
| `InvalidDomain` | 无效域名（长度或非法字符） |
| `DomainNotFound` | 域不存在 |
| `DomainAlreadyExists` | 域已存在（重复注册） |
| `DomainPinDisabled` | 域的自动 PIN 已禁用 |
| `SubjectNotFound` | Subject 未找到（CID 无归属） |

## 事件

### Pin 生命周期
| 事件 | 说明 |
|------|------|
| `PinRequested` | 请求已受理（cid_hash, payer, replicas, size, price） |
| `PinSubmitted` | 已提交到 IPFS Cluster |
| `PinMarkedPinned` | 标记 Pin 成功（cid_hash, replicas） |
| `PinMarkedFailed` | 标记 Pin 失败（cid_hash, code） |
| `PinStateChanged` | Pin 状态迁移（cid_hash, state） |
| `PinCharged` | 完成一次周期扣费（cid_hash, amount, period, next_charge_at） |
| `PinGrace` | 余额不足进入宽限期 |
| `PinExpired` | 超出宽限期，标记过期 |
| `PinRenewed` | Pin 已续期（cid_hash, periods, total_fee） |
| `PinTierUpgraded` | Pin 等级升级（old_tier, new_tier, fee_diff） |
| `PinRemoved` | CID 已从 IPFS 物理删除（cid_hash, reason） |
| `OrphanCidDetected` | on_idle 扫描到孤儿 CID，已标记 unpin（cid_hash） |
| `MarkedForUnpin` | CID 已标记为待 Unpin |
| `UnpinRefund` | 提前取消按比例退款（cid_hash, owner, refund） |
| `BatchUnpinCompleted` | 批量取消完成（who, requested, unpinned） |

### 运营者
| 事件 | 说明 |
|------|------|
| `OperatorJoined` | 运营者注册（operator, capacity_gib, bond） |
| `OperatorUpdated` | 运营者信息更新 |
| `OperatorLeft` | 运营者离开（进入宽限期） |
| `OperatorUnregistered` | 运营者注销完成（保证金已退还） |
| `OperatorStatusChanged` | 运营者状态变更（operator, old, new） |
| `OperatorSlashed` | 运营者保证金被扣罚（operator, amount） |
| `OperatorAutoSuspended` | 运营者因健康分过低被自动暂停 |
| `OperatorCapacityWarning` | 运营者容量使用超 80% 告警 |
| `OperatorHealthDegraded` | 运营者健康度下降超 10 分 |
| `OperatorLayerUpdated` | 运营者层级变更（operator, layer, priority） |
| `OperatorPinSuccess` | 运营者 Pin 成功（operator, cid_hash） |
| `OperatorPinFailed` | 运营者 Pin 失败（operator, cid_hash, reason） |
| `OperatorPinsMigrated` | Pin 迁移完成（from, to, pins_migrated, bytes_moved） |
| `BondTopUp` | 保证金追加（operator, amount, new_total） |
| `BondReduced` | 保证金减少（operator, amount, new_total） |
| `RewardsClaimed` | 运营者领取奖励 |

### 计费
| 事件 | 说明 |
|------|------|
| `ChargedFromIpfsPool` | 从 IPFS 池扣费（subject_id, amount, remaining_quota） |
| `ChargedFromSubjectFunding` | 从用户资金账户扣费 |
| `ChargedFromEntityTreasury` | 从 Entity 国库扣费（entity_id, amount） |
| `IpfsPoolLowBalanceWarning` | IPFS 池余额不足告警 |
| `GracePeriodStarted` | 宽限期开始 |
| `RewardDistributed` | 奖励分配完成（total_amount, operator_count） |
| `OperatorRewarded` | 运营者获得奖励（operator, amount, weight） |
| `BillingPausedByGovernance` | 治理暂停计费 |
| `BillingResumedByGovernance` | 治理恢复计费 |
| `BillingParamsUpdated` | 计费参数已更新 |
| `ReplicasConfigUpdated` | 副本数配置已更新 |
| `IpfsPoolFunded` | IPFS 公共池已充值（who, amount, new_balance） |
| `UserFunded` | 用户资金账户已充值 |
| `SubjectFunded` | Subject 资金账户已充值 |

### 分层与健康
| 事件 | 说明 |
|------|------|
| `TierConfigUpdated` | 分层配置更新 |
| `HealthCheckCompleted` | 健康巡检完成（cid_hash, status） |
| `PinAssignedToOperator` | Pin 分配到运营者 |
| `LayeredPinAssigned` | 分层 Pin 分配完成（core/community operators） |
| `StorageLayerConfigUpdated` | 分层策略配置更新 |
| `CoreOperatorShortage` | Layer 1 运营者不足告警（required, available） |
| `CommunityOperatorShortage` | Layer 2 运营者不足告警 |
| `SimplePinAllocated` | 简化 PIN 分配完成（cid_hash, tier, nodes） |
| `SimplePinStatusReported` | PIN 状态报告（OCW 上报） |
| `SimpleNodeLoadWarning` | 节点负载告警（capacity_usage, current_pins） |

### 域管理
| 事件 | 说明 |
|------|------|
| `DomainRegistered` | 域已注册（domain, subject_type_id） |
| `DomainConfigUpdated` | 域配置已更新（domain, auto_pin_enabled） |
| `DomainPrioritySet` | 域优先级已设置（domain, priority） |
| `DomainStatsUpdated` | 域统计已更新（total_pins, healthy/degraded/critical） |
| `ContentRegisteredViaDomain` | 内容通过域注册（domain, subject_id, cid_hash, tier） |
| `GovernanceForceUnpinned` | 治理强制下架 CID（cid_hash, reason） |

### CID 锁定
| 事件 | 说明 |
|------|------|
| `CidLocked` | CID 已锁定（cid_hash, until） |
| `CidUnlocked` | CID 已解锁（cid_hash） |

## ValidateUnsigned

仅接受 `Local` 或 `InBlock` 来源的 OCW unsigned 交易：

| Call | 验证规则 |
|------|---------|
| `ocw_mark_pinned` | CID 在 PendingPins 中 + 运营者 Active + 防重放(区块号) |
| `ocw_mark_pin_failed` | 同上 |
| `ocw_submit_assignments` | CID 在 PendingPins 中 + 无现有分配 + 防重放 |
| `ocw_report_health` | 运营者 Active + CID 存在 + 运营者已分配到该CID + 防重放 |

## 集成示例

### 方式一：StoragePin（所有业务 pallet 统一使用）
```rust
pub trait Config: frame_system::Config {
    type StoragePin: StoragePin<Self::AccountId>;
}

// domain 字符串自动映射到 DomainPins 域索引和 SubjectType
// entity_id: Some(eid) 用于 Entity 国库扣费层，None 跳过
T::StoragePin::pin(who, b"evidence", evidence_id, None, cid_vec, PinTier::Critical)?;
T::StoragePin::unpin(who, cid_vec)?;
```

### 方式二：CidLockManager（仲裁场景）
```rust
pub trait Config: frame_system::Config {
    type CidLockManager: CidLockManager<Self::Hash, BlockNumberFor<Self>>;
}

// 锁定 CID 防止删除
T::CidLockManager::lock_cid(cid_hash, b"arbitration:123".to_vec(), Some(expiry))?;
```

## RPC 查询接口

| 方法 | 说明 |
|------|------|
| `get_domain_stats(domain)` | 查询指定域的统计信息 |
| `get_all_domain_stats()` | 查询所有域统计（按优先级排序） |
| `get_domain_cids(domain, offset, limit)` | 分页查询域的 CID 列表 |
| `due_at_count(block)` | 查询某块的到期扣费数量 |
| `due_between(from, to)` | 查询区间内的到期分布 |

## 公共辅助函数

### 账户派生
| 函数 | 说明 |
|------|------|
| `derive_user_funding_account(user)` | 派生用户级资金账户（推荐） |
| `derive_subject_funding_account_v2(type, id)` | 按 SubjectType 派生资金账户 |
| `subject_account_for(domain, id)` | ⚠️ 已废弃，用 `derive_user_funding_account` |

### 运营者查询
| 函数 | 说明 |
|------|------|
| `get_operator_metrics(operator)` | 聚合运营者多维指标（供 RPC） |
| `calculate_health_score(operator)` | 健康度评分算法（0-100） |
| `calculate_capacity_usage(operator)` | 容量使用率（0-100%） |
| `check_operator_capacity_warning(operator)` | 容量超 80% 自动告警 |
| `count_operator_pins(operator)` | O(1) 查询 Pin 数量 |

### Pin 管理
| 函数 | 说明 |
|------|------|
| `check_pin_health(cid_hash)` | 检查 Pin 健康状态 |
| `get_pin_operators(cid_hash)` | 获取 Pin 运营者列表 |
| `get_tier_config(tier)` | 获取分层配置（含默认值） |
| `get_recommended_replicas(level)` | 获取推荐副本数（level 0-3） |

### 费用计算
| 函数 | 说明 |
|------|------|
| `calculate_initial_pin_fee(size, replicas)` | 计算初始 Pin 费用（30天预扣） |
| `calculate_period_fee(size, replicas)` | 计算单周期费用 |
| `four_layer_charge(cid_hash, task)` | 五层扣费机制核心（配额 → Entity 国库 → UserFunding → IpfsPool → 宽限期） |

### 运营者选择
| 函数 | 说明 |
|------|------|
| `select_operators_by_layer(type, tier)` | 分层智能选择运营者 |
| `optimized_pin_allocation(...)` | 基于容量+健康度的节点选择（OCW） |

## 文件结构

| 文件 | 说明 |
|------|------|
| `src/lib.rs` | 主模块：Config、Storage、Events、Errors、Extrinsics、Hooks |
| `src/types.rs` | 类型定义：SubjectType、PinTier、TierConfig、BillingTask 等 |
| `src/weights.rs` | 权重定义 |
| `src/runtime_api.rs` | Runtime API 定义 |
| `src/benchmarking.rs` | 基准测试 |
| `src/tests.rs` | 单元测试 |

## 审计历史

### Round 4（2026年3月）
| 级别 | ID | 说明 | 状态 |
|------|-----|------|------|
| Critical | C1-R4 | `renew_pin` 收费但不延长 BillingQueue | ✅ 已修复 |
| Critical | C2-R4 | `upgrade_pin_tier` 不更新 BillingQueue 的 amount_per_period | ✅ 已修复 |
| High | H1-R4 | `on_finalize` 任务3.5 全表扫描 PinBilling::iter() | ✅ 已修复（ExpiredCidQueue） |
| Medium | M1-R4 | `request_pin_for_subject` 冗余双重分配 | 📝 记录 |
| Medium | M2-R4 | `ContentRegistry::register_content` 用 IpfsPool 作为 owner | 📝 记录 |
| Medium | M3-R4 | 所有权校验失败返回 BadParams 而非 NotOwner | ✅ 已修复 |
| Low | L1-R4 | CID 长度校验晚于计算 | 📝 记录 |
| Low | L2-R4 | 宽限期超时仍有 active pins | 📝 记录 |

### Round 5（2026年3月）
| 级别 | ID | 说明 | 状态 |
|------|-----|------|------|
| Critical | C1-R5 | Pool 资金不安全：OperatorRewards 可超过 Pool 余额 | ✅ 已修复（reserve 锁定） |
| Critical | C2-R5 | `renew_pin`/`upgrade_pin_tier` O(N) 线性扫描 BillingQueue | ✅ 已修复（CidBillingDueBlock 反向索引） |
| High | H1-R5 | 退款使用硬编码28天，不关联实际预付/续费 | ✅ 已修复（基于 BillingQueue 实际状态） |
| High | H2-R5 | README 宽限期描述360天与代码7天/3天不一致 | ✅ 已修复 |
| Medium | M1-R5 | 宽限期固定1h重试（7天内168次），链上开销大 | ✅ 已修复（指数退避1h→24h） |
| Medium | M2-R5 | 配额按 subject_id 粒度，多Subject可滥用 | ✅ 已修复（改为用户级 AccountId） |
| Low | L1-R5 | GiB 向上取整对小文件严重不公 | ✅ 已修复（MiB 级计费） |
| Low | L2-R5 | 运营者等额分配无质量激励 | ✅ 已修复（health_score 加权） |

### Round 6（2026年3月）
| 级别 | ID | 说明 | 状态 |
|------|-----|------|------|
| High | H1-R6 | Entity 封禁/关闭时 Product CID 未级联 unpin | ✅ 已修复（ProductProvider::force_unpin_shop_products） |
| Medium | M1-R6 | 过期CID清理遗漏 CidEntityOf、PendingPins | ✅ 已修复 |
| Medium | M2-R6 | HealthCheckQueue 幽灵条目持续空转 | ✅ 已修复（PinMeta guard） |
| Medium | M3-R6 | 孤儿 CID 无发现机制 | ✅ 已修复（on_idle sweep + OrphanSweepCursor） |

## 相关模块

- `pallets/storage/lifecycle/` — 存储生命周期管理
- `pallets/dispute/evidence/` — 证据存证（依赖 IpfsPinner）
- `pallets/trading/otc/` — OTC 交易（依赖 IpfsPinner）
- `pallets/dispute/arbitration/` — 仲裁（依赖 CidLockManager）
