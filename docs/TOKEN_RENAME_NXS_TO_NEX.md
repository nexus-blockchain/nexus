# 代币重命名: NEX → NEX 修改流程

## 1. 影响范围统计

| 类别 | 文件数 | 匹配数 | 风险等级 |
|------|--------|--------|---------|
| Rust 源码 (.rs) | 29 | 485 | 🔴 高 |
| TypeScript 脚本 (.ts) | 18 | 152 | 🟡 中 |
| 文档 (.md) | 24 | 335 | 🟢 低 |
| Cargo.toml | 1 | 1 | 🟢 低 |
| **合计** | **72** | **973** | - |

---

## 2. 修改分类

### 2.1 链级配置 (Critical — 必须正确)

| 文件 | 修改内容 | 说明 |
|------|---------|------|
| `node/src/chain_spec.rs` | `"tokenSymbol": "NEX"` → `"NEX"` | 链元数据 token symbol |
| `runtime/build.rs` | `.enable_metadata_hash("NEX", 12)` → `"NEX"` | WASM 元数据哈希 |

### 2.2 Rust 代码标识符 (High — 需全量替换)

分三类替换模式:

#### A. 变量名 / 字段名中的 `nxs` (小写, 117+101=~200处)

| 模式 | 替换为 | 示例 | 涉及文件 |
|------|--------|------|---------|
| `nex_amount` | `nex_amount` | 订单金额字段 | p2p, market, pricing, maker |
| `nex_balance` | `nex_balance` | 余额变量 | p2p, market, pricing |
| `nex_price` | `nex_price` | 价格变量 | pricing, market |
| `nex_total` | `nex_total` | 汇总字段 | p2p, market |
| `nex_fee` | `nex_fee` | 手续费 | p2p, market |
| `nex_value` | `nex_value` | 价值变量 | pricing |
| `_nxs` 后缀 | `_nex` | `amount_nxs`, `price_nxs` | market, pricing, p2p |
| `to_nxs` / `from_nxs` | `to_nex` / `from_nex` | 转换函数 | pricing |

#### B. 枚举变体 / 类型名中的 `Nxs` (CamelCase)

| 模式 | 替换为 | 示例 | 涉及文件 |
|------|--------|------|---------|
| `NxsToUsdt` | `NexToUsdt` | TradeDirection 枚举 | common/traits, p2p |
| `UsdtToNxs` | `UsdtToNex` | TradeDirection 枚举 | common/traits, p2p |
| `NxsChannel` | `NexChannel` | 交易通道枚举 | market |
| `NxsBalance` | `NexBalance` | 余额类型 | market |

#### C. 注释 / 字符串中的 `NEX` (大写)

| 模式 | 替换为 | 示例 |
|------|--------|------|
| `// ... NEX ...` | `// ... NEX ...` | 注释说明 |
| `"NEX"` 字符串字面量 | `"NEX"` | chain_spec, build.rs |
| `NEX/USDT` | `NEX/USDT` | 交易对名称 |
| `10 NEX` | `10 NEX` | 金额说明 |

### 2.3 涉及的 Rust 文件完整清单 (29个)

**Trading 模块 (13 文件, ~350 处):**
| 文件 | 匹配数 | 主要内容 |
|------|--------|---------|
| `pallets/trading/p2p/src/lib.rs` | 89 | 字段名 nex_amount/nex_fee/nex_total |
| `pallets/trading/pricing/src/lib.rs` | 83 | nex_price, to_nxs, NEX/USDT 汇率 |
| `pallets/trading/common/src/traits.rs` | 30 | NxsToUsdt/UsdtToNxs 枚举, nxs_ trait 方法 |
| `pallets/trading/pricing/src/tests.rs` | 41 | 测试中的 NEX 金额 |
| `pallets/trading/maker/src/lib.rs` | 16 | nex_balance 字段 |
| `pallets/trading/p2p/src/mock.rs` | 12 | mock 中的 NEX 配置 |
| `pallets/trading/p2p/src/tests.rs` | 9 | 测试中的 NEX 金额 |
| `pallets/trading/p2p/src/types.rs` | 4 | nex_amount 字段定义 |
| `pallets/trading/p2p/src/weights.rs` | 2 | 注释 |
| `pallets/trading/p2p/Cargo.toml` | 1 | 注释 |
| `pallets/trading/credit/src/lib.rs` | 2 | NEX 注释 |
| `pallets/trading/credit/src/buyer.rs` | 1 | NEX 注释 |
| `pallets/trading/credit/src/tests.rs` | 1 | NEX 注释 |

**Entity 模块 (11 文件, ~90 处):**
| 文件 | 匹配数 | 主要内容 |
|------|--------|---------|
| `pallets/entity/market/src/lib.rs` | 77 | NxsChannel, nex_balance, _nxs 变量 |
| `pallets/entity/market/src/tests.rs` | 20 | 测试 NEX 金额 |
| `pallets/entity/market/src/mock.rs` | 5 | mock NEX 配置 |
| `pallets/entity/tokensale/src/lib.rs` | 21 | NEX 支付相关 |
| `pallets/entity/tokensale/src/tests.rs` | 3 | 测试 |
| `pallets/entity/registry/src/lib.rs` | 16 | nxs_ 操作资金字段 |
| `pallets/entity/registry/src/mock.rs` | 3 | mock |
| `pallets/entity/service/src/lib.rs` | 10 | nxs_ 金额字段 |
| `pallets/entity/service/src/tests.rs` | 3 | 测试 |
| `pallets/entity/service/src/mock.rs` | 1 | mock |
| `pallets/entity/common/src/lib.rs` | 5 | NEX 通用类型 |

**Storage 模块 (2 文件, 12 处):**
| 文件 | 匹配数 |
|------|--------|
| `pallets/storage/service/src/lib.rs` | 4 |
| `pallets/storage/service/src/tests.rs` | 8 |

**Runtime / Node (3 文件, 18 处):**
| 文件 | 匹配数 | 主要内容 |
|------|--------|---------|
| `runtime/src/configs/mod.rs` | 16 | 注释中 "NEX"、UNIT 用量 |
| `runtime/build.rs` | 1 | metadata hash |
| `node/src/chain_spec.rs` | 1 | tokenSymbol |
| `runtime/src/genesis_config_presets.rs` | 1 | INITIAL_SUPPLY 注释 |

### 2.4 TypeScript 脚本 (18 文件, 152 处)

| 文件 | 匹配数 | 主要内容 |
|------|--------|---------|
| `scripts/e2e/flows/trading/p2p-buy.ts` | 22 | NEX 金额打印 |
| `scripts/test-pricing.ts` | 15 | NEX/USDT 价格测试 |
| `scripts/test-cny-rate.ts` | 14 | NEX/CNY 汇率 |
| `scripts/e2e/flows/trading/p2p-sell.ts` | 13 | NEX 金额 |
| `scripts/test-otc.ts` | 12 | NEX 交易 |
| `scripts/transfer.ts` | 11 | NEX 转账打印 |
| `scripts/create-test-accounts.ts` | 10 | NEX 余额 |
| `scripts/test-swap.ts` | 10 | NEX 交换 |
| `scripts/transfer-to-accounts.ts` | 10 | NEX 转账 |
| `scripts/utils/helpers.ts` | 7 | formatNxs / NEX helper |
| `scripts/e2e/core/config.ts` | 6 | NEX 常量 |
| `scripts/test-escrow.ts` | 5 | NEX 金额 |
| `scripts/test-maker.ts` | 5 | NEX 金额 |
| `scripts/activate-maker.ts` | 3 | NEX 打印 |
| `scripts/approve-maker.ts` | 3 | NEX 打印 |
| `scripts/e2e/fixtures/accounts.ts` | 3 | NEX 余额 |
| `scripts/e2e/flows/entity/entity-shop.ts` | 2 | NEX 金额 |
| `scripts/e2e/core/assertions.ts` | 1 | NEX 断言 |

### 2.5 文档 (.md) (24 文件, 335 处)

主要文件:
| 文件 | 匹配数 |
|------|--------|
| `pallets/trading/pricing/README.md` | 60 |
| `docs/COMMUNITY_POINTS_GOVERNANCE.md` | 53 |
| `docs/NEXUS_NODE_REWARD_DESIGN.md` | 33 |
| `pallets/entity/market/README.md` | 27 |
| `pallets/trading/p2p/README.md` | 23 |
| 其他 19 个文件 | 139 |

---

## 3. 执行计划 (7 个 Step)

### Step 1: 链级配置 (2 处, 5分钟)
**风险: 🔴 最高 — 影响链身份**

```
node/src/chain_spec.rs:  "tokenSymbol": "NEX" → "NEX"
runtime/build.rs:        .enable_metadata_hash("NEX", 12) → ("NEX", 12)
```

验证: `cargo check -p nexus-runtime && cargo check -p nexus-node`

### Step 2: 公共类型 + Trait (trading/common, entity/common)
**风险: 🔴 高 — 下游全部依赖**

目标文件:
- `pallets/trading/common/src/traits.rs` (30处) — 枚举 NxsToUsdt→NexToUsdt, trait 方法签名
- `pallets/entity/common/src/lib.rs` (5处) — 公共类型

**替换规则:**
| 查找 | 替换 |
|------|------|
| `NxsToUsdt` | `NexToUsdt` |
| `UsdtToNxs` | `UsdtToNex` |
| `nex_amount` | `nex_amount` |
| `nex_balance` | `nex_balance` |
| `nex_price` | `nex_price` |
| `nex_fee` | `nex_fee` |
| `nex_total` | `nex_total` |
| `nex_value` | `nex_value` |
| `// ... NEX ...` (注释) | `// ... NEX ...` |

验证: `cargo check -p pallet-trading-common && cargo check -p pallet-entity-common`

### Step 3: Trading 模块 Pallet 代码 (10 文件)
**风险: 🟡 中 — 单模块替换**

按依赖顺序:
1. `pallets/trading/pricing/src/lib.rs` (83处)
2. `pallets/trading/pricing/src/tests.rs` (41处)
3. `pallets/trading/maker/src/lib.rs` (16处)
4. `pallets/trading/credit/src/lib.rs` + `buyer.rs` + `tests.rs` (4处)
5. `pallets/trading/p2p/src/lib.rs` (89处)
6. `pallets/trading/p2p/src/types.rs` (4处)
7. `pallets/trading/p2p/src/mock.rs` (12处)
8. `pallets/trading/p2p/src/tests.rs` (9处)
9. `pallets/trading/p2p/src/weights.rs` (2处)
10. `pallets/trading/p2p/Cargo.toml` (1处)

每个 pallet 完成后验证: `cargo check -p <pallet-name>`

### Step 4: Entity 模块 Pallet 代码 (11 文件)
**风险: 🟡 中**

按依赖顺序:
1. `pallets/entity/common/src/lib.rs` (Step 2 已完成)
2. `pallets/entity/registry/src/lib.rs` + `mock.rs` (19处)
3. `pallets/entity/market/src/lib.rs` + `mock.rs` + `tests.rs` (102处)
4. `pallets/entity/tokensale/src/lib.rs` + `tests.rs` (24处)
5. `pallets/entity/service/src/lib.rs` + `mock.rs` + `tests.rs` (14处)

### Step 5: Storage + Runtime (6 文件)
**风险: 🟡 中**

1. `pallets/storage/service/src/lib.rs` + `tests.rs` (12处)
2. `runtime/src/configs/mod.rs` (16处) — 注释中的 "NEX" → "NEX"
3. `runtime/src/genesis_config_presets.rs` (1处) — 注释

### Step 6: TypeScript 脚本 (18 文件)
**风险: 🟢 低 — 非编译目标**

全局替换规则:
| 查找 | 替换 |
|------|------|
| `NEX` (大写, 字符串/注释) | `NEX` |
| `nxs` (小写, 变量名) | `nex` |
| `formatNxs` | `formatNex` |
| `NEX/USDT` | `NEX/USDT` |
| `NEX/CNY` | `NEX/CNY` |

### Step 7: 文档 (24 文件)
**风险: 🟢 最低**

全局 `NEX` → `NEX` 替换 (注意不要误替换 "NEXUS" 中的子串，但 NEX 不是 NEXUS 的子串，安全)。

---

## 4. 替换安全性分析

### 4.1 安全的全局替换

| 模式 | 安全性 | 原因 |
|------|--------|------|
| `"NEX"` → `"NEX"` | ✅ 安全 | 字符串字面量，精确匹配 |
| `NxsToUsdt` → `NexToUsdt` | ✅ 安全 | 唯一枚举名 |
| `UsdtToNxs` → `UsdtToNex` | ✅ 安全 | 唯一枚举名 |
| `nex_amount` → `nex_amount` | ✅ 安全 | 唯一标识符 |
| `nex_balance` → `nex_balance` | ✅ 安全 | 唯一标识符 |
| `nex_price` → `nex_price` | ✅ 安全 | 唯一标识符 |
| `nex_fee` → `nex_fee` | ✅ 安全 | 唯一标识符 |
| `NEX ` (大写+空格, 注释) | ✅ 安全 | NEX 不是任何其他词的子串 |

### 4.2 需要注意的模式

| 模式 | 风险 | 处理方式 |
|------|------|---------|
| `NEX` 在 `NEXUS` 中? | 无风险 | NEX ≠ NEXUS 子串 |
| `nexus-runtime` crate 名 | 无关 | 不含 NEX |
| `PalletId(*b"xxx")` | 检查 | 确认无 NEX 在 PalletId 中 |

### 4.3 不修改的内容

- **crate 名称**: `nexus-runtime`, `nexus-node` 等 — 这些包含 "nexus" 不含 "nxs"
- **UNIT / MILLI_UNIT / MICRO_UNIT 常量**: 这些是通用名不含 NEX
- **PalletId**: 使用 8 字节标识符如 `*b"et/sale/"`, 不含 NEX

---

## 5. 链上存储兼容性

### 5.1 主网未上线 — 无迁移需求 ✅

由于主网尚未上线，此次重命名:
- **无需 Storage Migration** — 没有历史数据
- **无需双符号过渡期** — 没有用户资产
- **无需 runtime upgrade 兼容** — 重新 genesis 即可

### 5.2 如果已上线需额外处理 (记录备查)

- Chain spec 的 `tokenSymbol` 变更需 runtime upgrade
- 前端/钱包/区块浏览器需同步更新
- 交易所需通知代币符号变更
- SCALE 编码的枚举变体名变更不影响链上存储 (SCALE 按索引编码)

---

## 6. 验证清单

### 编译验证
```bash
# Step 1-5 完成后
cargo check -p nexus-runtime
cargo check -p nexus-node

# 全量编译
cargo build --release
```

### 测试验证
```bash
# 逐 pallet 测试
cargo test -p pallet-trading-common
cargo test -p pallet-trading-pricing
cargo test -p pallet-trading-p2p
cargo test -p pallet-entity-market
cargo test -p pallet-entity-tokensale
cargo test -p pallet-entity-service
cargo test -p pallet-entity-registry
cargo test -p pallet-storage-service
# ... 其他 pallet

# 全量测试
cargo test --workspace
```

### 搜索验证 (确认无遗漏)
```bash
# 确认 Rust 源码中不再有 NEX (除注释引用历史名称)
grep -rn "NEX\|nxs_\|_nxs\|Nxs" --include="*.rs" .

# 确认 TS 中不再有 NEX
grep -rn "NEX\|nxs" --include="*.ts" .

# 确认文档中不再有 NEX
grep -rn "NEX" --include="*.md" .
```

---

## 7. 预估工时

| Step | 描述 | 预估时间 |
|------|------|---------|
| Step 1 | 链级配置 (2处) | 5 分钟 |
| Step 2 | 公共类型 + Trait (35处) | 15 分钟 |
| Step 3 | Trading 模块 (261处) | 40 分钟 |
| Step 4 | Entity 模块 (159处) | 30 分钟 |
| Step 5 | Storage + Runtime (29处) | 10 分钟 |
| Step 6 | TypeScript 脚本 (152处) | 20 分钟 |
| Step 7 | 文档 (335处) | 20 分钟 |
| 验证 | 编译 + 测试 | 30 分钟 |
| **合计** | | **~170 分钟 (~3小时)** |

---

## 8. 执行方式建议

推荐使用 **sed 批量替换 + 逐步验证** 的方式:

```bash
# 示例: 精确替换变量名 (安全, 不会误伤)
find . -name "*.rs" -exec sed -i 's/nex_amount/nex_amount/g' {} +
find . -name "*.rs" -exec sed -i 's/nex_balance/nex_balance/g' {} +
find . -name "*.rs" -exec sed -i 's/nex_price/nex_price/g' {} +
find . -name "*.rs" -exec sed -i 's/nex_fee/nex_fee/g' {} +
find . -name "*.rs" -exec sed -i 's/nex_total/nex_total/g' {} +
find . -name "*.rs" -exec sed -i 's/nex_value/nex_value/g' {} +
find . -name "*.rs" -exec sed -i 's/NxsToUsdt/NexToUsdt/g' {} +
find . -name "*.rs" -exec sed -i 's/UsdtToNxs/UsdtToNex/g' {} +
find . -name "*.rs" -exec sed -i 's/NxsChannel/NexChannel/g' {} +
find . -name "*.rs" -exec sed -i 's/NxsBalance/NexBalance/g' {} +
find . -name "*.rs" -exec sed -i 's/"NEX"/"NEX"/g' {} +

# 注释中的 NEX → NEX (需人工审核少量边界情况)
find . -name "*.rs" -exec sed -i 's/NEX/NEX/g' {} +

# TypeScript
find scripts/ -name "*.ts" -exec sed -i 's/NEX/NEX/g; s/nxs/nex/g; s/formatNxs/formatNex/g' {} +

# 文档
find . -name "*.md" -exec sed -i 's/NEX/NEX/g' {} +
```

**注意:** 执行 sed 替换时应先 `git stash` 或创建分支，便于回滚。

---

*文档生成时间: 2026-02-22*
*当前状态: 主网未上线，无链上数据迁移需求*
