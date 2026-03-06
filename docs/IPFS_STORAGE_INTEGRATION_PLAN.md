# IPFS 存储集成完善方案

> 日期：2026-03-05
> 状态：规划中

## 一、全量模块分析

### 1.1 项目 Pallet 全景图

```
pallets/
├── entity/                        # 实体生态系统（14 个子模块）
│   ├── registry/                  # 实体生命周期
│   ├── shop/                      # 店铺管理
│   ├── product/                   # 商品管理
│   ├── order/                     # 订单生命周期
│   ├── review/                    # 订单评价
│   ├── disclosure/                # 信息披露
│   ├── governance/                # DAO 治理
│   ├── kyc/                       # KYC/AML
│   ├── member/                    # 会员与推荐
│   ├── token/                     # 实体代币（pallet-assets 桥接）
│   ├── tokensale/                 # 代币发售
│   ├── market/                    # 实体代币 P2P 市场
│   ├── common/                    # 共享类型与 Trait
│   └── commission/                # 佣金系统（8 个子模块）
│       ├── core/                  # 佣金引擎
│       ├── referral/              # 推荐佣金
│       ├── multi-level/           # 多层级佣金
│       ├── single-line/           # 单线佣金
│       ├── level-diff/            # 级差佣金
│       ├── team/                  # 团队业绩佣金
│       ├── pool-reward/           # 池奖励
│       └── common/                # 佣金共享类型
├── trading/                       # NEX/USDT 交易（3 个子模块）
│   ├── nex-market/                # NEX↔USDT P2P 市场
│   ├── trc20-verifier/            # TRC20 链上验证（OCW）
│   └── common/                    # 交易共享类型
├── dispute/                       # 争议与仲裁（3 个子模块）
│   ├── arbitration/               # 投诉与仲裁
│   ├── evidence/                  # 证据管理
│   └── escrow/                    # 资金托管
├── storage/                       # IPFS 存储基础设施（2 个子模块）
│   ├── service/                   # Pin/Unpin/CID 锁定/运营者管理
│   └── lifecycle/                 # 归档生命周期（Active→L1→L2→Purge）
└── ads/                           # 广告系统（4 个子模块）
    ├── core/                      # 广告引擎
    ├── entity/                    # 实体广告位
    ├── router/                    # 投放路由
    └── primitives/                # 广告共享类型
```

### 1.2 CID 使用情况分类

#### 已集成 IPFS Pin/Unpin/Lock（3 个模块）

| 模块 | CID 字段 | 已集成能力 | 集成方式 |
|------|----------|-----------|---------|
| `entity/product` | name_cid, images_cid, detail_cid, tags_cid, sku_cid | Pin + Unpin | `IpfsPinner` |
| `dispute/evidence` | content_cid | Pin | `IpfsPinner` |
| `dispute/arbitration` | — | 证据 CID Lock/Unlock | `CidLockManager` |

#### 仅存储 CID，未 Pin（9 个模块，11 个业务域）

| 模块 | CID 字段 | 数据重要性 | 遗漏风险 |
|------|----------|-----------|---------|
| `entity/registry` | logo_cid, description_cid, contact_cid, metadata_uri | 高 | 实体元数据 GC 丢失 |
| `entity/shop` | logo_cid, description_cid, address_cid, business_hours_cid, policies_cid | 高 | 店铺元数据 GC 丢失 |
| `entity/disclosure` | content_cid, summary_cid（披露）；content_cid（公告） | 高 | 合规披露内容丢失 |
| `entity/governance` | description_cid + ProposalType 内 20+ CID 字段 | 高 | 治理提案内容丢失 |
| `entity/kyc` | data_cid, rejection_details_cid | 极高 | 认证材料丢失 |
| `entity/review` | content_cid（评价）；content_cid（回复） | 中 | 评价内容丢失 |
| `entity/order` | shipping_cid, tracking_cid, note_cid | 中 | 物流/备注信息丢失 |
| `dispute/arbitration` | details_cid, response_cid, settlement_cid, resolution_cid | 极高 | 仲裁文书丢失 |
| `trading/nex-market` | evidence_cid（交易争议） | 高 | 交易争议证据丢失 |

#### 无 CID 使用（18 个模块，无需集成）

| 类别 | 模块 |
|------|------|
| 实体 | member, token, tokensale, market, common |
| 佣金 | core, referral, multi-level, single-line, level-diff, team, pool-reward, common |
| 交易 | trc20-verifier, common |
| 争议 | escrow |
| 广告 | core, entity, router, primitives |

### 1.3 核心问题

| # | 问题 | 影响 | 涉及模块 |
|---|------|------|---------|
| P1 | **CID 孤儿风险** | 链上记录 CID 但 IPFS 节点未 Pin，数据随时被 GC 丢失 | 9 个未集成模块 |
| P2 | **无生命周期 Unpin** | 实体/店铺关闭、订单终态后 CID 未 Unpin，浪费存储 | registry, shop, order |
| P3 | **SubjectType 缺失** | 仅有 Evidence/Product 等 8 种，缺少 7 种业务域类型 | storage/service |
| P4 | **仲裁文书未 Pin** | arbitration 自身的 details/response/settlement/resolution_cid 仅锁定不 Pin | arbitration |
| P5 | **OnEntityStatusChange 未接入** | 已有级联通知 trait 但未用于 IPFS CID 清理 | entity/common |
| P6 | **FeeType::IpfsPin 死代码** | entity-registry 定义了该费用类型但从未实际扣费 | registry |
| P7 | **争议期 CID 无保护** | order 争议期间的 CID 无锁定保护 | order, nex-market |

### 1.4 已有基础设施

#### storage/service 提供的 Trait

| Trait | 方法 | 当前使用方 |
|-------|------|-----------|
| `IpfsPinner<AccountId, Balance>` | `pin_cid_for_subject(caller, subject_type, subject_id, cid, tier)` / `unpin_cid(caller, cid)` | product, evidence |
| `ContentRegistry` | `register_content(domain, subject_id, cid, tier)` / `unregister_content(domain, cid)` | **无**（推荐新模块使用） |
| `CidLockManager<Hash, BlockNumber>` | `lock_cid(cid_hash, reason, until)` / `unlock_cid(cid_hash, reason)` / `is_locked(cid_hash)` | arbitration |

#### IpfsPinner 四层扣费机制

```
1. IpfsPoolAccount 配额 → 2. SubjectFunding(subject_id) → 3. IpfsPoolAccount 兜底 → 4. GracePeriod 宽限
```

#### entity/common 已有级联通知 Trait

```rust
pub trait OnEntityStatusChange {
    fn on_entity_suspended(entity_id: u64);
    fn on_entity_banned(entity_id: u64);
    fn on_entity_resumed(entity_id: u64);
    fn on_entity_closed(entity_id: u64);
}
```

#### storage/lifecycle 已有归档管线

```
Active → ArchivedL1（精简存储） → ArchivedL2（元数据保留） → Purged（完全删除）
```
- 支持按数据类型差异化 `ArchivePolicy`（l1_delay / l2_delay / purge_delay）
- 支持 `PurgeProtection`（保护重要数据免于清除）
- 支持 `extend_active_period`（延长 Active 期）
- `on_hooks` 每块自动扫描执行

---

## 二、SubjectType 扩展

现有 `SubjectType` 枚举（`storage/service/src/types.rs`）需新增 7 种业务域：

```rust
pub enum SubjectType {
    // ===== 现有 =====
    Evidence,       // domain=0
    OtcOrder,       // domain=1
    Chat,           // domain=5
    Livestream,     // domain=6
    Swap,           // domain=7
    Arbitration,    // domain=8
    UserProfile,    // domain=9
    Product,        // domain=10
    // ===== 新增 =====
    Entity,         // domain=11, 实体元数据
    Shop,           // domain=12, 店铺元数据
    Disclosure,     // domain=13, 信息披露 + 公告
    Review,         // domain=14, 评价 + 回复
    Order,          // domain=15, 订单附件
    Governance,     // domain=16, 治理提案
    Kyc,            // domain=17, KYC 认证数据
    // ==============
    General,        // domain=98
    Custom(BoundedVec<u8, ConstU32<32>>), // domain=99
}
```

### 新增 SubjectType 默认存储策略

| SubjectType | PinTier | 副本数 | 归档策略 | 说明 |
|-------------|---------|--------|---------|------|
| Entity | Standard | 3 | L1=90d, L2=180d, Purge=365d | 实体存续期间持久保存 |
| Shop | Standard | 3 | L1=90d, L2=180d, Purge=365d | 店铺存续期间持久保存 |
| Disclosure | Standard | 3 | 无 Purge | 合规要求长期可查 |
| Review | Temporary | 1 | L1=60d, L2=120d, Purge=180d | 评价数据量大，按需保留 |
| Order | Temporary | 1 | L1=30d, Purge=90d | 订单完成后可降级 |
| Governance | Standard | 3 | 无 Purge | 提案内容需长期可查 |
| Kyc | Critical | 5 | 无 Purge, PurgeProtection | KYC 数据加密且重要 |

---

## 三、分阶段实施方案

### Phase 1：核心实体层（P1-High）

**目标**：为实体和店铺元数据 CID 集成 IpfsPinner，实现关闭时自动 Unpin。

#### 1.1 entity-registry 集成 IpfsPinner

**改动文件**：
- `pallets/entity/registry/src/lib.rs` — Config 新增 `type IpfsPinner`
- `pallets/entity/registry/src/lifecycle.rs` — 创建/更新/关闭逻辑增加 Pin/Unpin
- `pallets/entity/registry/src/helpers.rs` — 新增 `pin_entity_cid` / `unpin_entity_cid` 辅助函数
- `runtime/src/configs/mod.rs` — 注入 `IpfsPinner = pallet_storage_service::Pallet<Runtime>`

**CID 字段与操作映射**：

| 操作 | logo_cid | description_cid | contact_cid | metadata_uri |
|------|----------|----------------|-------------|-------------|
| `create_entity` | Pin | Pin | — | — |
| `update_entity` | ΔPin | ΔPin | ΔPin | ΔPin |
| `do_approve_close_entity` | Unpin | Unpin | Unpin | Unpin |
| `do_execute_close_timeout` | Unpin | Unpin | Unpin | Unpin |
| `do_ban_entity` | Unpin | Unpin | Unpin | Unpin |

> ΔPin = CID 变更时 Unpin 旧值 + Pin 新值；CID 设为 None 时仅 Unpin

**策略**：`SubjectType::Entity`, `subject_id = entity_id`, `PinTier::Standard`, best-effort

**FeeType::IpfsPin 激活**：Pin 时通过 `IpfsPinner` 四层扣费自动扣除，FeeType 用于事件审计。

#### 1.2 entity-shop 集成 IpfsPinner

**改动文件**：
- `pallets/entity/shop/src/lib.rs` — Config 新增 `type IpfsPinner`

**CID 字段与操作映射**：

| 操作 | logo_cid | description_cid | address_cid | business_hours_cid | policies_cid |
|------|----------|----------------|-------------|-------------------|-------------|
| `do_create_shop` | — | — | — | — | — |
| `update_shop_info` | ΔPin | ΔPin | — | — | — |
| `update_shop_location` | — | — | ΔPin | ΔPin | — |
| `set_business_hours` | — | — | — | ΔPin | — |
| `set_shop_policies` | — | — | — | — | ΔPin |
| 店铺关闭/ban | Unpin | Unpin | Unpin | Unpin | Unpin |

> `do_create_shop` 内部调用，创建时各 CID 字段为 None，无需 Pin

**策略**：`SubjectType::Shop`, `subject_id = shop_id`, `PinTier::Standard`, best-effort

**Option\<Option\<CID\>\> 处理**：shop 的 update 使用三态语义（None=不修改, Some(None)=清除, Some(Some(cid))=设新值），辅助函数需适配。

---

### Phase 2：关键内容层（P2-Medium）

**目标**：为高价值/合规要求的业务内容集成 Pin。

#### 2.1 entity-disclosure 集成 IpfsPinner

| 操作 | content_cid | summary_cid |
|------|-------------|-------------|
| `publish_disclosure` | Pin | Pin |
| `create_draft` | Pin | Pin |
| `update_draft` | ΔPin | ΔPin |
| `correct_disclosure`（更正） | Pin 新记录 | Pin 新记录 |
| `publish_announcement` | Pin content_cid | — |

> `correct_disclosure` 创建新记录引用旧记录，旧记录 CID **不 Unpin**（保留历史审计链）

**策略**：`SubjectType::Disclosure`, `PinTier::Standard`

#### 2.2 entity-governance 集成 IpfsPinner

| 操作 | Pin | Unpin |
|------|-----|-------|
| `create_proposal` | description_cid + ProposalType 内所有 CID | — |
| 提案执行/过期/拒绝 | — | 可选 Unpin（默认保留审计） |

**ProposalType CID 提取**：

`ProposalType` 含 20+ 变体，CID 分布不均。需提取通用辅助函数：

```rust
fn extract_cids_from_proposal_type(
    pt: &ProposalType<Balance>,
) -> Vec<&BoundedVec<u8, ConstU32<64>>> {
    match pt {
        ProposalType::ProductListing { product_cid } => vec![product_cid],
        ProposalType::ShopDescriptionChange { description_cid } => vec![description_cid],
        ProposalType::TokenMint { recipient_cid, .. } => vec![recipient_cid],
        ProposalType::AirdropDistribution { airdrop_cid, .. } => vec![airdrop_cid],
        ProposalType::TreasurySpend { recipient_cid, reason_cid, .. } => vec![recipient_cid, reason_cid],
        ProposalType::RefundPolicy { policy_cid } => vec![policy_cid],
        ProposalType::WithdrawalConfigChange { tier_configs_cid, .. } => vec![tier_configs_cid],
        ProposalType::AddUpgradeRule { rule_cid } => vec![rule_cid],
        ProposalType::CommunityEvent { event_cid } => vec![event_cid],
        ProposalType::RuleSuggestion { suggestion_cid } => vec![suggestion_cid],
        ProposalType::General { title_cid, content_cid } => vec![title_cid, content_cid],
        _ => vec![], // 其余变体无 CID 字段
    }
}
```

**策略**：`SubjectType::Governance`, `PinTier::Standard`

#### 2.3 entity-kyc 集成 IpfsPinner

| 操作 | data_cid | rejection_details_cid |
|------|----------|----------------------|
| `submit_kyc` | Pin | — |
| `update_kyc_data` | ΔPin | — |
| `reject_kyc` | — | Pin |
| `grant_kyc`（内部） | — | — |
| `purge_kyc_data` | Unpin | Unpin |

**策略**：`SubjectType::Kyc`, `PinTier::Critical`（KYC 数据加密存储，最高安全级别，5 副本）

**隐私说明**：KYC data_cid 内容已加密，Pin 到 IPFS 不泄露明文。结合 `PinTier::Critical` 仅分发到 Core 层运营者节点，进一步缩小数据暴露面。

#### 2.4 dispute/arbitration 集成 IpfsPinner（补全 Pin 缺口）

**现状问题**：arbitration 已集成 `CidLockManager` 锁定证据 CID，但其自身 Complaint 结构体的 4 个 CID 字段从未 Pin。

| 操作 | details_cid | response_cid | settlement_cid | resolution_cid |
|------|-------------|-------------|---------------|---------------|
| `do_submit_complaint` | Pin | — | — | — |
| `respond_to_complaint` | — | Pin | — | — |
| `settle_complaint` | — | — | Pin | — |
| `resolve_complaint` / `arbitrate` | — | — | — | Pin |
| 案件关闭（过期/超时） | — | — | — | — |

> 仲裁文书属于法律级别数据，**不自动 Unpin**，走 lifecycle 归档流程。

**策略**：`SubjectType::Arbitration`（复用现有），`PinTier::Critical`

#### 2.5 trading/nex-market 争议证据 Pin

| 操作 | evidence_cid |
|------|-------------|
| `dispute_trade` | Pin |
| `resolve_dispute` | — （不 Unpin，保留审计） |

**策略**：`SubjectType::OtcOrder`（复用现有），`PinTier::Standard`

---

### Phase 3：业务附件层（P3-Medium）

**目标**：为订单附件和评价内容集成 Pin，实现终态 Unpin。

#### 3.1 entity-review 集成 IpfsPinner

| 操作 | content_cid |
|------|-------------|
| `submit_review` | Pin（可选字段，有值则 Pin） |
| `edit_review` | ΔPin |
| `reply_to_review` | Pin reply.content_cid |
| `force_remove_review` | Unpin review + reply content_cid |

**策略**：`SubjectType::Review`, `PinTier::Temporary`（评价数据量大，降低存储成本）

#### 3.2 entity-order 集成 IpfsPinner

**CID 字段分析**：

| CID 字段 | 存储位置 | 生命周期 |
|----------|---------|---------|
| shipping_cid | Order 结构体 | 跟随订单 |
| tracking_cid | Order 结构体 | 跟随订单 |
| note_cid | Order 结构体 | 跟随订单 |
| reason_cid | 仅用于 extrinsic 参数（event 审计），**未持久化到 Order 结构体** | 不适用 |

| 操作 | shipping_cid | tracking_cid | note_cid |
|------|-------------|-------------|---------|
| `create_order` | Pin | — | Pin |
| `ship_order` | — | Pin | — |
| `update_shipping_address` | ΔPin | — | — |
| `update_tracking_info` | — | ΔPin | — |
| 终态（Completed/Refunded/Cancelled） | Unpin | Unpin | Unpin |

> `request_refund` / `reject_refund` / `force_refund` 的 reason_cid 仅为 extrinsic 参数用于事件审计，未存入 Order 结构体，暂不 Pin。如后续需要持久化，应先扩展 Order 结构体再集成。

**策略**：`SubjectType::Order`, `PinTier::Temporary`

---

### Phase 4：生命周期管理（P2-Medium）

**目标**：实体/店铺级联关闭时自动 Unpin 所有关联 CID。

#### 4.1 利用 OnEntityStatusChange 实现级联 Unpin

`entity/common` 已定义 `OnEntityStatusChange` trait，可直接用于级联 IPFS 清理：

```rust
pub trait OnEntityStatusChange {
    fn on_entity_suspended(entity_id: u64);
    fn on_entity_banned(entity_id: u64);
    fn on_entity_resumed(entity_id: u64);
    fn on_entity_closed(entity_id: u64);
}
```

**级联 Unpin 范围**（实体 Close/Ban 时）：

| 层级 | 模块 | Unpin 的 CID | 触发方式 |
|------|------|-------------|---------|
| L0 | entity-registry | logo, description, contact, metadata_uri | 直接 Unpin |
| L1 | entity-shop | 所有关联 shop 的 5 个 CID | `on_entity_closed` 回调 |
| L2 | entity-product | 所有关联 product 的 5 个 CID | **已实现**（product 有独立 Unpin） |
| L3 | entity-disclosure | 所有关联 disclosure/announcement 的 CID | `on_entity_closed` 回调 |

**实现方式**：

使用 tuple 组合多个实现者注入 runtime：

```rust
type OnEntityStatusChange = (
    pallet_entity_shop::Pallet<Runtime>,
    pallet_entity_disclosure::Pallet<Runtime>,
    // product 已有独立清理逻辑
);
```

各模块实现 `OnEntityStatusChange`，在 `on_entity_closed` / `on_entity_banned` 中遍历自身关联记录并 Unpin。

#### 4.2 订单终态 Unpin 策略

| CID | 即时 Unpin | 延迟 Unpin | 说明 |
|-----|-----------|-----------|------|
| shipping_cid | 是 | — | 用户私人地址，终态后无需保留 |
| note_cid | 是 | — | 买家备注，终态后无需保留 |
| tracking_cid | — | 争议窗口期后 | 物流信息在争议窗口期内需可查 |

**延迟 Unpin 配置**：新增 Config 常量 `OrderCidRetentionBlocks`（默认 7 天），tracking_cid 在终态 + 保留期后 Unpin。

#### 4.3 接入 storage/lifecycle 归档管线

将各业务 CID 注册到 `pallet-storage-lifecycle` 的 `ArchivePolicy`：

| 数据类型 | L1 延迟 | L2 延迟 | Purge 延迟 | Purge 开关 | PurgeProtection |
|---------|---------|---------|-----------|-----------|----------------|
| `b"entity"` | 90 天 | 180 天 | 365 天 | 开 | 否 |
| `b"shop"` | 90 天 | 180 天 | 365 天 | 开 | 否 |
| `b"disclosure"` | 365 天 | 730 天 | — | **关** | — |
| `b"review"` | 60 天 | 120 天 | 180 天 | 开 | 否 |
| `b"order"` | 30 天 | 60 天 | 90 天 | 开 | 否 |
| `b"governance"` | 365 天 | 730 天 | — | **关** | — |
| `b"kyc"` | 730 天 | — | — | **关** | **是** |
| `b"arbitration"` | 365 天 | 730 天 | — | **关** | — |

> Disclosure、Governance、KYC、Arbitration 属于合规/法律数据，禁用 Purge。

---

### Phase 5：争议保护层（P3-Low）

**目标**：争议期间锁定相关业务 CID 防止被删除。

#### 5.1 entity-order 争议期 CID 锁定

| 触发 | 锁定 CID | 解锁时机 |
|------|---------|---------|
| `request_refund` | shipping_cid, tracking_cid, note_cid | 退款完成/拒绝 |
| `reject_refund` → 争议窗口 | 同上 | 争议窗口期结束 |
| 升级到 arbitration | 由 arbitration 接管锁定 | 仲裁裁决 |

**实现**：entity-order Config 新增 `type CidLockManager`

#### 5.2 nex-market 争议期 CID 锁定

| 触发 | 锁定 CID | 解锁时机 |
|------|---------|---------|
| `dispute_trade` | evidence_cid | 争议解决 |

**实现**：nex-market Config 新增 `type CidLockManager`

---

## 四、通用辅助模式

### 4.1 best-effort Pin/Unpin 辅助函数模板

所有模块统一采用 best-effort 模式（Pin/Unpin 失败不阻断业务，参照 product 实现）：

```rust
fn pin_cid(
    caller: &T::AccountId,
    subject_type: SubjectType,
    subject_id: u64,
    cid: &BoundedVec<u8, T::MaxCidLength>,
    tier: PinTier,
) {
    if cid.is_empty() { return; }
    let cid_vec: Vec<u8> = cid.clone().into_inner();
    let size_estimate = cid_vec.len() as u64 * 1024;
    if let Err(e) = T::IpfsPinner::pin_cid_for_subject(
        caller.clone(), subject_type, subject_id,
        cid_vec, size_estimate, Some(tier),
    ) {
        log::warn!(target: LOG_TARGET, "Pin failed for subject {}: {:?}", subject_id, e);
    }
}

fn unpin_cid(caller: &T::AccountId, cid: &BoundedVec<u8, T::MaxCidLength>) {
    if cid.is_empty() { return; }
    let cid_vec: Vec<u8> = cid.clone().into_inner();
    if let Err(e) = T::IpfsPinner::unpin_cid(caller.clone(), cid_vec) {
        log::warn!(target: LOG_TARGET, "Unpin failed: {:?}", e);
    }
}
```

### 4.2 Option\<BoundedVec\> CID 处理

多数模块 CID 字段为 `Option<BoundedVec<u8, MaxCidLen>>`：

```rust
fn pin_optional_cid(
    caller: &T::AccountId,
    subject_type: SubjectType,
    id: u64,
    cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    tier: PinTier,
) {
    if let Some(c) = cid {
        Self::pin_cid(caller, subject_type, id, c, tier);
    }
}

fn unpin_optional_cid(
    caller: &T::AccountId,
    cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
) {
    if let Some(c) = cid {
        Self::unpin_cid(caller, c);
    }
}
```

### 4.3 CID 更新模式（ΔPin = Unpin 旧 + Pin 新）

```rust
fn update_cid_pin(
    caller: &T::AccountId,
    subject_type: SubjectType,
    subject_id: u64,
    old_cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    new_cid: &Option<BoundedVec<u8, T::MaxCidLength>>,
    tier: PinTier,
) {
    if old_cid != new_cid {
        Self::unpin_optional_cid(caller, old_cid);
        Self::pin_optional_cid(caller, subject_type, subject_id, new_cid, tier);
    }
}
```

### 4.4 Shop 三态 Option\<Option\<CID\>\> 适配

Shop 的 update 使用 `Option<Option<BoundedVec>>` 语义（None=不修改, Some(None)=清除, Some(Some(cid))=设新值），需额外适配：

```rust
fn handle_triple_option_cid(
    caller: &T::AccountId,
    subject_type: SubjectType,
    subject_id: u64,
    current: &Option<BoundedVec<u8, T::MaxCidLength>>,
    input: &Option<Option<BoundedVec<u8, T::MaxCidLength>>>,
    tier: PinTier,
) {
    match input {
        Some(Some(new_cid)) => {
            Self::unpin_optional_cid(caller, current);
            Self::pin_cid(caller, subject_type, subject_id, new_cid, tier);
        }
        Some(None) => {
            Self::unpin_optional_cid(caller, current);
        }
        None => {} // 不修改，不操作
    }
}
```

### 4.5 ProposalType 批量 Pin 辅助

```rust
fn pin_proposal_cids(
    caller: &T::AccountId,
    proposal_id: u64,
    proposal_type: &ProposalType<BalanceOf<T>>,
) {
    for cid in Self::extract_cids_from_proposal_type(proposal_type) {
        if !cid.is_empty() {
            let cid_vec: Vec<u8> = cid.clone().into_inner();
            let size_estimate = cid_vec.len() as u64 * 1024;
            let _ = T::IpfsPinner::pin_cid_for_subject(
                caller.clone(),
                SubjectType::Governance,
                proposal_id,
                cid_vec,
                size_estimate,
                Some(PinTier::Standard),
            );
        }
    }
}
```

---

## 五、Runtime 配置变更

### 5.1 各模块 Config 新增项

| 模块 | 新增 Config | 注入值 |
|------|------------|-------|
| entity-registry | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| entity-shop | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| entity-disclosure | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| entity-review | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| entity-order | `type IpfsPinner` + `type CidLockManager` | `pallet_storage_service::Pallet<Runtime>` |
| entity-governance | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| entity-kyc | `type IpfsPinner` | `pallet_storage_service::Pallet<Runtime>` |
| dispute-arbitration | `type IpfsPinner`（新增，CidLockManager 已有） | `pallet_storage_service::Pallet<Runtime>` |
| trading/nex-market | `type IpfsPinner` + `type CidLockManager` | `pallet_storage_service::Pallet<Runtime>` |

### 5.2 Cargo.toml 依赖

各模块需添加：

```toml
[dependencies]
pallet-storage-service = { path = "../../storage/service", default-features = false }

[features]
std = ["pallet-storage-service/std"]
```

### 5.3 OnEntityStatusChange 注入扩展

当前 runtime 中 `OnEntityStatusChange` 需追加 CID 清理实现者：

```rust
type OnEntityStatusChange = (
    ExistingHandlers,
    pallet_entity_shop::IpfsCleanup<Runtime>,
    pallet_entity_disclosure::IpfsCleanup<Runtime>,
);
```

---

## 六、测试计划

每个模块需新增的测试类型：

| 测试类型 | 说明 |
|---------|------|
| Pin 正向 | 创建/更新操作后验证 Pin 调用（mock IpfsPinner） |
| Unpin 正向 | 删除/关闭后验证 Unpin 调用 |
| best-effort | Pin/Unpin 失败不阻断业务流程 |
| ΔPin 更新 | 变更 CID 时旧值 Unpin + 新值 Pin |
| 级联关闭 | 实体关闭时所有子模块 CID 被 Unpin |
| 争议锁定 | 争议期间 CID 被锁定，解决后解锁 |
| 幂等性 | 重复 Pin 同一 CID 不报错（CidAlreadyPinned 静默） |

### 预估测试数

| Phase | 涉及模块 | 预估测试数 |
|-------|---------|-----------|
| Phase 1: 核心实体层 | registry, shop | ~16 |
| Phase 2: 关键内容层 | disclosure, governance, kyc, arbitration, nex-market | ~30 |
| Phase 3: 业务附件层 | review, order | ~14 |
| Phase 4: 生命周期管理 | registry(级联), shop(级联), lifecycle(策略) | ~12 |
| Phase 5: 争议保护层 | order(锁定), nex-market(锁定) | ~8 |
| **合计** | **9 个模块** | **~80** |

---

## 七、实施优先级与工期估算

| Phase | 优先级 | 预估工期 | 前置依赖 |
|-------|--------|---------|---------|
| SubjectType 扩展 | P0 | 0.5 天 | 无 |
| Phase 1: registry + shop | P1-High | 2 天 | SubjectType |
| Phase 2: disclosure + governance + kyc + arbitration + nex-market | P2-Medium | 3.5 天 | SubjectType |
| Phase 3: review + order | P2-Medium | 2 天 | SubjectType |
| Phase 4: 生命周期管理 | P2-Medium | 2 天 | Phase 1 + 2 + 3 |
| Phase 5: 争议 CID 锁定 | P3-Low | 1 天 | Phase 3 |
| **合计** | — | **~11 天** | — |

---

## 八、风险与注意事项

| # | 风险 | 缓解措施 |
|---|------|---------|
| R1 | **存储迁移** | SubjectType 新增变体为追加枚举，不破坏 SCALE 解码，无需数据迁移 |
| R2 | **Pin 费用来源** | 所有模块 Pin 费用走 `IpfsPinner` 四层扣费，不需各模块单独处理费用逻辑 |
| R3 | **best-effort 原则** | 所有 Pin/Unpin 操作均不阻断业务，失败仅记录日志 |
| R4 | **幂等性** | 重复 Pin 同一 CID 由 `CidAlreadyPinned` 保护，best-effort 模式下静默忽略 |
| R5 | **KYC 隐私** | data_cid 内容已加密，`PinTier::Critical` + 仅 Core 层运营者存储，CID 本身链上可见 |
| R6 | **size_estimate 精度** | 当前采用 `cid.len() * 1024` 估算，长期应由前端/OCW 上报实际大小 |
| R7 | **Governance CID 碎片化** | ProposalType 20+ 变体各有不同 CID 字段，通过 `extract_cids_from_proposal_type` 统一处理 |
| R8 | **Order reason_cid 未持久化** | reason_cid 仅为 extrinsic 参数，不在 Order 结构体中，暂不 Pin。如需持久化应先做结构体迁移 |
| R9 | **Arbitration CID 不 Unpin** | 仲裁文书为法律级别数据，仅走 lifecycle 归档，不主动 Unpin，需确保 lifecycle 正确配置 |
| R10 | **级联 Unpin 性能** | 大型实体关闭时可能触发大量 Unpin，应采用分批处理或异步（OCW）模式避免超出区块权重限制 |

---

## 附录 A：完整 CID 字段清单

| 模块 | 结构体/参数 | CID 字段 | 类型 | 需 Pin | 需 Unpin | 需 Lock |
|------|-----------|---------|------|--------|---------|---------|
| registry | Entity | logo_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| registry | Entity | description_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| registry | Entity | contact_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| registry | Entity | metadata_uri | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| shop | Shop | logo_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| shop | Shop | description_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| shop | Shop | address_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| shop | Shop | business_hours_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| shop | Shop | policies_cid | `Option<BoundedVec>` | Phase 1 | Phase 1/4 | — |
| product | Product | name_cid 等 5 个 | `BoundedVec` | **已实现** | **已实现** | — |
| disclosure | DisclosureRecord | content_cid | `BoundedVec` | Phase 2 | — | — |
| disclosure | DisclosureRecord | summary_cid | `Option<BoundedVec>` | Phase 2 | — | — |
| disclosure | AnnouncementRecord | content_cid | `BoundedVec` | Phase 2 | — | — |
| governance | Proposal | description_cid | `Option<BoundedVec>` | Phase 2 | — | — |
| governance | ProposalType(20+ 变体) | 各种 *_cid | `BoundedVec<u8, 64>` | Phase 2 | — | — |
| kyc | KycRecord | data_cid | `Option<BoundedVec>` | Phase 2 | Phase 2 | — |
| kyc | KycRecord | rejection_details_cid | `Option<BoundedVec>` | Phase 2 | Phase 2 | — |
| arbitration | Complaint | details_cid | `BoundedVec` | Phase 2 | — | — |
| arbitration | Complaint | response_cid | `Option<BoundedVec>` | Phase 2 | — | — |
| arbitration | Complaint | settlement_cid | `Option<BoundedVec>` | Phase 2 | — | — |
| arbitration | Complaint | resolution_cid | `Option<BoundedVec>` | Phase 2 | — | — |
| nex-market | TradeDispute | evidence_cid | `BoundedVec<u8, 128>` | Phase 2 | — | Phase 5 |
| review | MallReview | content_cid | `Option<BoundedVec>` | Phase 3 | Phase 3 | — |
| review | ReviewReply | content_cid | `BoundedVec` | Phase 3 | Phase 3 | — |
| order | Order | shipping_cid | `Option<BoundedVec>` | Phase 3 | Phase 3 | Phase 5 |
| order | Order | tracking_cid | `Option<BoundedVec>` | Phase 3 | Phase 3 | Phase 5 |
| order | Order | note_cid | `Option<BoundedVec>` | Phase 3 | Phase 3 | Phase 5 |
| evidence | Evidence | content_cid | `BoundedVec` | **已实现** | — | — |
