# Dispute 模块组

> 路径：`pallets/dispute/`

NEXUS 争议解决基础设施，提供资金托管、证据管理、仲裁裁决功能，支持 8 个业务域的争议与投诉处理。

## 目录结构

```
pallets/dispute/
├── escrow/       # 资金托管 (pallet-escrow, index 60)
├── evidence/     # 证据管理 (pallet-evidence, index 61)
└── arbitration/  # 仲裁裁决 + 统一投诉 (pallet-arbitration, index 62)
```

## 子模块

| 模块 | 功能 | Runtime Index | 依赖 |
|------|------|:---:|------|
| **escrow** | 资金锁定、释放、退款、分账、到期策略 | 60 | 无 |
| **evidence** | 证据提交、IPFS Pin、私密内容、访问控制 | 61 | pallet-storage-service |
| **arbitration** | 仲裁（双向押金）+ 统一投诉系统 | 62 | escrow, evidence, pallet-trading-common |

## 依赖关系

```
                   ┌─────────────────┐
                   │   Arbitration   │
                   │ 仲裁 + 投诉系统  │
                   └──────┬──────────┘
                          │
            ┌─────────────┼─────────────┐
            ▼             ▼             ▼
      ┌──────────┐  ┌──────────┐  ┌──────────────────┐
      │  Escrow  │  │ Evidence │  │ trading-common    │
      │ 资金托管  │  │ 证据管理  │  │ PricingProvider  │
      └──────────┘  └──────────┘  └──────────────────┘
```

## 业务域（8 字节标识）

| 域标识 | 业务 | 域标识 | 业务 |
|--------|------|--------|------|
| `otc_ord_` | OTC 交易 | `livstrm_` | 直播 |
| `maker___` | 做市商 | `nft_trd_` | NFT 交易 |
| `swap____` | Swap 交换 | `member__` | 会员 |
| `credit__` | 信用系统 | `other___` | 其他 |

## 两大子系统

### 仲裁系统（资金争议，双向押金）

```
1. evidence::commit                        → 发起方提交证据
2. arbitration::dispute_with_two_way_deposit → 发起仲裁（锁发起方押金 15%）
3. evidence::commit                        → 应诉方提交证据
4. arbitration::respond_to_dispute          → 应诉（锁应诉方押金 15%）
5. arbitration::arbitrate                   → 治理裁决 (Release/Refund/Partial)
   → escrow 分账 + 押金罚没/释放 + CID 解锁 + 信用分更新
```

### 投诉系统（行为投诉，25 种类型）

```
1. arbitration::file_complaint             → 发起投诉（锁 ~1 USDT 押金）
2. arbitration::respond_to_complaint        → 被投诉方申诉
3. arbitration::settle_complaint            → 和解（或）
   arbitration::escalate_to_arbitration     → 升级到仲裁委员会
4. arbitration::resolve_complaint           → 治理裁决
```

投诉状态机：
```
Submitted → Responded → Mediating → Arbitrating → Resolved (Win/Lose/Settlement)
    │                        │
    └→ Withdrawn             └→ Expired
```

## 安全审计修复

| 编号 | 模块 | 修复内容 |
|------|------|---------|
| EH3 | escrow | `lock`/`lock_with_nonce` 拒绝已关闭的托管 |
| EM1 | escrow | `schedule_expiry` 使用 `ExpiringAtFull` 错误码 |
| EM2 | escrow | `release`/`refund`/`release_split` 返回 `DisputeActive` |
| VC1 | evidence | 5 种类型添加 `DecodeWithMemTracking` |
| VC2 | evidence | 5 个 extrinsic 使用正确 WeightInfo |
| VH1/VH2 | evidence | `commit`/`append_evidence` 添加边界验证 |
| VM1 | evidence | 密钥轮换计数使用 O(1) `KeyRotationCounter` |
| VM2 | evidence | 归档清理关联索引 |
| AC1 | arbitration | 3 种类型添加 `DecodeWithMemTracking` |
| AC2 | arbitration | 7 个 weight 函数独立化 |
| AH1 | arbitration | `append_evidence_id` 增加权限校验 |
| AH4 | arbitration | 投诉过期使用游标扫描（O(batch) vs O(N)） |
| AH5/AH6/AH7 | arbitration | 撤诉/和解/过期时退还投诉押金 |

## 相关文档

- [escrow/README.md](escrow/README.md) — 资金托管详情
- [evidence/README.md](evidence/README.md) — 证据管理详情
- [arbitration/README.md](arbitration/README.md) — 仲裁 + 投诉详情
