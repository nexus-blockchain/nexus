# Storage Pallets

存储服务模块组，包含 IPFS 存储管理和数据生命周期管理功能。

## 模块结构

```
storage/
├── service/     # 存储服务核心 (pallet-storage-service)
└── lifecycle/   # 存储生命周期管理 (pallet-storage-lifecycle)
```

## 模块说明

### service (存储服务)

**功能**：
- IPFS Pin 管理（创建、续期、取消）
- 存储运营商注册与管理
- 计费与配额系统
- 健康巡检队列

**主要类型**：
- `PinRecord` - Pin 记录
- `Operator` - 运营商信息
- `BillingRecord` - 计费记录

### lifecycle (生命周期管理)

**功能**：
- 分级归档框架 (Active → L1 → L2 → Purge)
- 自动过期处理
- 可配置的归档延迟

**主要 Trait**：
- `ArchivableData` - 可归档数据接口

## 配置参数

### StorageService
| 参数 | 说明 |
|------|------|
| `MinOperatorBond` | 运营商最低押金 |
| `MinCapacityGiB` | 最小存储容量 |
| `DefaultBillingPeriod` | 默认计费周期 |

### StorageLifecycle
| 参数 | 说明 |
|------|------|
| `L1ArchiveDelay` | Active → L1 延迟 |
| `L2ArchiveDelay` | L1 → L2 延迟 |
| `PurgeDelay` | L2 → Purge 延迟 |
| `EnablePurge` | 是否启用清除 |

## 依赖关系

```
Evidence ──────► StorageService (IpfsPinner)
Arbitration ───► StorageService + StorageLifecycle
Trading/OTC ───► StorageService + StorageLifecycle
Trading/Swap ──► StorageService + StorageLifecycle
```

## 安全审计 (2026-02-23)

### pallet-storage-service 修复项

| ID | 级别 | 描述 | 状态 |
|----|------|------|------|
| C1 | Critical | OCW storage writes are silent no-ops (architectural, needs unsigned tx) | 📋 Documented |
| C2 | Critical | `four_layer_charge` double-spend: withdraw burns tokens + operator claims again | ✅ Fixed |
| C3 | Critical | `charge_due` calls `distribute_to_pin_operators` again after `four_layer_charge` already did | ✅ Fixed |
| H1 | High | `count_operator_pins` O(N) full table scan → `OperatorPinCount` storage map O(1) | ✅ Fixed |
| H6 | Medium | 20+ extrinsics used hardcoded `weight(10_000)` → proper `T::WeightInfo::*()` calls | ✅ Fixed |
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

### 新增存储项

- `OperatorPinCount<T>`: `StorageMap<AccountId, u32>` — O(1) operator pin count index

### 新增 Config 常量

- `OperatorGracePeriod`: operator unregistration grace period (runtime: 7 days)

### 新增 WeightInfo 函数 (21 total)

`join_operator`, `update_operator`, `leave_operator`, `set_operator_status`, `report_probe`,
`slash_operator`, `fund_subject_account`, `fund_user_account`, `set_replicas_config`,
`distribute_to_operators`, `set_storage_layer_config`, `set_operator_layer`, `pause_operator`,
`resume_operator`, `update_tier_config`, `operator_claim_rewards`, `emergency_pause_billing`,
`resume_billing`, `register_domain`, `update_domain_config`, `request_unpin`, `set_domain_priority`

### 验证

- `cargo check -p pallet-storage-service` ✅
- `cargo check -p pallet-storage-lifecycle` ✅
- `cargo check -p nexus-runtime` ✅
- `cargo test -p pallet-storage-service --lib` ✅ 13/13 pass
