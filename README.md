# Nexus Blockchain

Nexus 是一个基于 [Polkadot SDK (Substrate)](https://github.com/nicories/polkadot-sdk) 构建的 Layer-1 区块链，提供去中心化商业实体管理、P2P 交易、佣金分销、争议仲裁、以及 TEE 去中心化社群机器人（GroupRobot）等完整 Web3 商业基础设施。

## 系统架构

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Nexus 生态系统                               │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │                     Substrate 区块链 (L1)                        │  │
│  │  node/ (节点)  +  runtime/ (运行时, 40+ pallets)                 │  │
│  │  Aura 出块 · GRANDPA 终局 · 6s 出块 · Wasm Runtime              │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│         │                    │                     │                   │
│  ┌──────┴──────┐   ┌────────┴────────┐   ┌───────┴────────────┐     │
│  │ Entity 商业  │   │  P2P 交易系统    │   │  GroupRobot 系统    │     │
│  │ 15 pallets  │   │  6 pallets       │   │  5 pallets (链上)  │     │
│  │ 实体·店铺·  │   │  做市·定价·信用  │   │  + grouprobot/     │     │
│  │ 代币·治理·  │   │  P2P·TRC20验证   │   │    (TEE 离链执行)   │     │
│  │ 会员·佣金   │   │                  │   │  TG + Discord 群管  │     │
│  └─────────────┘   └─────────────────┘   └────────────────────┘     │
│         │                    │                                        │
│  ┌──────┴──────┐   ┌────────┴────────┐                               │
│  │ 争议解决     │   │  去中心化存储    │                               │
│  │ 3 pallets   │   │  2 pallets       │                               │
│  │ 托管·证据·  │   │  IPFS 存储·     │                               │
│  │ 仲裁        │   │  生命周期管理    │                               │
│  └─────────────┘   └─────────────────┘                               │
└──────────────────────────────────────────────────────────────────────┘
```

## 核心功能

### Entity 商业系统 (15 个 Pallet)

完整的去中心化商业实体管理平台：

| 模块 | Pallet | 功能 |
|------|--------|------|
| **实体注册** | `entity-registry` | 实体创建、审核、暂停、关闭、转让 |
| **店铺管理** | `entity-shop` | 主店铺/子店铺、独立运营状态、统计级联 |
| **实体代币** | `entity-token` | 代币铸造、分红、锁仓、转账限制、白/黑名单 |
| **实体治理** | `entity-governance` | 提案、投票（时间加权）、委员会、否决权 |
| **会员体系** | `entity-member` | 多级会员、推荐链、自动升级、团队统计 |
| **佣金引擎** | `commission-core` + 4 插件 | 插件化分佣：推荐链、级差、单线、提现 |
| **服务管理** | `entity-service` | 服务/商品上架与管理 |
| **交易处理** | `entity-transaction` | 订单创建、支付、完成 |
| **评价系统** | `entity-review` | 订单评价、店铺评分聚合 |
| **内部市场** | `entity-market` | 实体代币 NEX/USDT 双通道交易、TWAP、熔断 |
| **信息披露** | `entity-disclosure` | 内幕人管理、交易窗口期控制 |
| **KYC** | `entity-kyc` | 多级身份认证 |
| **代币销售** | `entity-tokensale` | 多轮次 Token Sale、白名单、退款、提现 |

**佣金插件架构：**
- `commission-common` — 共享类型 + `CommissionPlugin` trait
- `commission-core` — 调度引擎、存储、提现（4 种模式）
- `commission-referral` — 5 种推荐链模式（直推/多级/固定/首单/复购）
- `commission-level-diff` — 级差佣金（全局 + 自定义等级）
- `commission-single-line` — 单线上下级佣金

### P2P 交易系统 (6 个 Pallet)

统一的 P2P 交易模块（合并 OTC 买单 + Swap 卖单）：

| Pallet | 功能 |
|--------|------|
| `trading-p2p` | 买单（法币→NEX）+ 卖单（NEX→法币），USDT 链上验证 |
| `trading-maker` | 做市商注册、审核、额度管理 |
| `trading-pricing` | CNY/USD 汇率预言机、价格保护 |
| `trading-credit` | 买方/卖方信用评分 |
| `trading-common` | 共享类型与接口 |
| `trading-trc20-verifier` | TRC20 USDT 链上交易验证（OCW） |

### GroupRobot — TEE 去中心化社群管理 (5 Pallet + 1 Binary)

在 TDX+SGX 可信执行环境中运行的多平台群管理系统：

**链上 Pallet (5 crate, index 150-153)：**

| Pallet | Index | 功能 |
|--------|:-----:|------|
| `grouprobot-primitives` | — | 共享类型 + Trait 定义 (BotRegistryProvider 等) |
| `grouprobot-registry` | 150 | Bot 注册、TEE 证明提交/刷新、MRTD/MRENCLAVE 白名单、平台绑定 |
| `grouprobot-consensus` | 151 | 节点注册/质押/退出、序列去重、订阅/Era 奖励、TEE 加权 |
| `grouprobot-community` | 152 | 群规则配置 (CAS 乐观锁)、动作日志提交 (单条/批量)、节点要求 |
| `grouprobot-ceremony` | 153 | RA-TLS 仪式记录/撤销、Enclave 审批、仪式过期检测 |

**离链执行程序 (`grouprobot/`)：**

独立 Rust 二进制 (47 源文件, 131 测试), 运行在 TEE 环境中:
- **平台支持**: Telegram (Webhook + Bot API) + Discord (Gateway WebSocket + REST API)
- **TEE 安全**: EnclaveBridge (Ed25519) + SealedStorage (AES-GCM) + TokenVault (Zeroizing+mlock)
- **密钥恢复**: Shamir K-of-N 分片 → Peer RA-TLS 收集 → 环境变量 fallback
- **证明**: TDX+SGX 双 Quote 生成, 24h 自动刷新
- **规则引擎**: 5 条可插拔规则链 (防刷屏/黑名单/命令/加群/默认)
- **链交互**: subxt 动态调用, ActionLogBatcher 批量提交 (6s 窗口)
- **进程隔离**: Vault IPC (加密 Unix socket) + Gramine SGX 部署

### 争议解决 (3 个 Pallet)

| Pallet | 功能 |
|--------|------|
| `escrow` | 多方资金托管、条件释放 |
| `evidence` | 链上证据存证（关联 IPFS CID） |
| `arbitration` | 仲裁委员会裁决、部分退款/全额退款/释放 |

### 去中心化存储 (2 个 Pallet)

| Pallet | 功能 |
|--------|------|
| `storage-service` | IPFS 文件存储注册与检索 |
| `storage-lifecycle` | 存储生命周期管理、过期清理 |

### 链上治理

4 个委员会实例（基于 `pallet-collective` + `pallet-membership`）：
- **技术委员会** — 协议升级与技术决策
- **仲裁委员会** — 争议裁决
- **财务委员会** — 国库资金管理
- **内容委员会** — 内容审核

## 项目结构

```
cosmos/
├── node/                        # Substrate 区块链节点
│   └── src/                     #   CLI、RPC、共识配置
├── runtime/                     # WASM 运行时 (40+ pallets)
│   └── src/
│       ├── lib.rs               #   区块参数、类型、pallet 注册
│       ├── configs/             #   各 pallet Config 实现
│       └── genesis_config_presets/ # 创世配置
├── pallets/                     # 自定义 Pallet 模块
│   ├── entity/                  #   Entity 商业系统 (15 pallets)
│   │   ├── common/              #     共享类型与 trait
│   │   ├── registry/            #     实体注册
│   │   ├── shop/                #     店铺管理
│   │   ├── token/               #     实体代币
│   │   ├── governance/          #     实体治理
│   │   ├── member/              #     会员体系
│   │   ├── commission/          #     佣金引擎 (core + 4 插件)
│   │   ├── service/             #     服务管理
│   │   ├── transaction/         #     交易处理
│   │   ├── market/              #     内部市场
│   │   ├── review/              #     评价系统
│   │   ├── disclosure/          #     信息披露
│   │   ├── kyc/                 #     KYC 认证
│   │   └── tokensale/           #     代币销售
│   ├── trading/                 #   P2P 交易系统 (6 pallets)
│   │   ├── common/              #     共享类型
│   │   ├── pricing/             #     汇率预言机
│   │   ├── credit/              #     信用评分
│   │   ├── maker/               #     做市商
│   │   ├── p2p/                 #     P2P 统一交易
│   │   └── trc20-verifier/      #     TRC20 验证 (OCW)
│   ├── dispute/                 #   争议解决 (3 pallets)
│   │   ├── escrow/              #     资金托管
│   │   ├── evidence/            #     证据存证
│   │   └── arbitration/         #     仲裁裁决
│   ├── storage/                 #   去中心化存储 (2 pallets)
│   │   ├── service/             #     IPFS 存储
│   │   └── lifecycle/           #     生命周期
│   └── grouprobot/              #   GroupRobot 链上 (5 crate)
│       ├── primitives/          #     共享类型与 Trait
│       ├── registry/            #     Bot 注册与 TEE 证明
│       ├── consensus/           #     节点共识与奖励
│       ├── community/           #     群规则与审计日志
│       └── ceremony/            #     RA-TLS 仪式审计
├── grouprobot/                  # GroupRobot TEE 离链执行 (独立二进制)
│   ├── src/                     #   47 源文件, 131 测试
│   │   ├── chain/               #     subxt 链客户端 (4 文件)
│   │   ├── tee/                 #     TEE 安全层 (13 文件)
│   │   ├── platform/            #     TG + Discord 适配 (6 文件)
│   │   ├── processing/          #     规则引擎 (8 文件)
│   │   └── infra/               #     指标/限流/缓存 (4 文件)
│   └── gramine/                 #   SGX Gramine 部署配置
├── frontend/                    # 前端 (预留)
├── scripts/                     # Polkadot.js 测试脚本
├── docs/                        # 架构文档
├── media-utils/                 # 媒体工具库
├── Cargo.toml                   # Workspace 配置
└── Dockerfile                   # 区块链节点 Docker 镜像
```

## Runtime Pallet 索引

| 索引 | Pallet | 类别 |
|------|--------|------|
| 0 | System | 基础 |
| 1 | Timestamp | 基础 |
| 2 | Aura | 共识 (出块) |
| 3 | Grandpa | 共识 (终局) |
| 4 | Balances | 基础 |
| 5 | TransactionPayment | 基础 |
| 6 | Sudo | 管理 |
| 50-52, 55 | TradingPricing / Credit / Maker / P2p | 交易 |
| 60, 62-65 | Escrow / StorageService / Evidence / Arbitration / StorageLifecycle | 争议与存储 |
| 70-77 | Technical / Arbitration / Treasury / Content Committee + Membership | 治理 |
| 90 | Contracts | 智能合约 |
| 110 | Assets | 资产 |
| 120-135 | EntityRegistry / Shop / Service / Transaction / Review / Token / Governance / Member / CommissionCore / Market / Disclosure / Kyc / TokenSale / Referral / LevelDiff / SingleLine | Entity 商业 |
| 150-153 | GroupRobotRegistry / Consensus / Community / Ceremony | GroupRobot |

## 快速开始

### 环境要求

- **Rust** stable（含 `wasm32-unknown-unknown` target）
- **Node.js** 18+（用于测试脚本）
- **Docker**（可选）

### 构建与运行

```bash
# 构建区块链节点
cargo build --release

# 启动开发链 (单节点模式)
./target/release/nexus-node --dev

# 连接 Polkadot.js Apps
# https://polkadot.js.org/apps/#/explorer?rpc=ws://localhost:9944
```

### 运行测试

```bash
# 运行全部 pallet 测试
cargo test

# 运行特定 pallet 测试
cargo test -p pallet-trading-p2p
cargo test -p pallet-entity-token
cargo test -p pallet-grouprobot-consensus

# GroupRobot 离链执行测试 (独立 workspace)
cd grouprobot && cargo test
```

### Docker 部署

```bash
# 构建区块链节点镜像
docker build . -t nexus-node

# 运行单节点开发链
docker run -p 9944:9944 -p 30333:30333 nexus-node --dev --rpc-external
```

## 链参数

| 参数 | 值 |
|------|-----|
| **代币符号** | NEX |
| **精度** | 12 位小数 (1 NEX = 10^12 单位) |
| **出块时间** | 6 秒 |
| **出块共识** | Aura |
| **终局共识** | GRANDPA |
| **SS58 格式** | 42 |
| **Existential Deposit** | 0.001 NEX |
| **Runtime 名称** | nexus |
| **Spec 版本** | 100 |

## 技术栈

### 区块链层
- **Polkadot SDK (Substrate)** — Runtime 框架
- **Rust** — 全部链上逻辑与节点实现
- **FRAME** — Pallet 开发框架
- **Aura + GRANDPA** — 混合共识
- **pallet-contracts** — ink! 智能合约支持

### GroupRobot 离链层
- **Axum 0.7** — HTTP 服务框架
- **Tokio** — 异步运行时
- **ed25519-dalek 2** — 消息签名
- **aes-gcm 0.10** — 密封存储 + IPC 加密
- **subxt 0.38** — Substrate 链交互（动态 API）
- **tokio-tungstenite** — Discord Gateway WebSocket
- **prometheus-client** — Prometheus 指标
- **tikv-jemallocator** — jemalloc (zero-on-free 安全分配)

## 文档

| 文档 | 路径 | 内容 |
|------|------|------|
| 开发路线图 | `docs/NEXUS_DEVELOPMENT_ROADMAP.md` | Sprint 规划与依赖关系 |
| GroupRobot 离链设计 | `docs/GROUPROBOT_OFFCHAIN_DESIGN.md` | 离链 TEE Bot 完整架构 |
| GroupRobot Pallet 设计 | `docs/GROUPROBOT_PALLET_DESIGN.md` | 链上 5 Pallet 设计文档 |
| GroupRobot 离链分析 | `docs/GROUPROBOT_OFFCHAIN_ANALYSIS.md` | 离链系统需求分析 |
| Bot Token 安全分析 | `docs/BOT_TOKEN_SECURITY_ANALYSIS.md` | Token 泄漏风险审计 |
| Ceremony 桥接方案 | `docs/PLAN_B_CEREMONY_RUNTIME_BRIDGE.md` | Shamir 仪式与运行时桥接 |
| TEE 对比分析 | `docs/TEE_BLOCKCHAIN_COMPARISON.md` | TEE+区块链方案对比 |
| TEE SGX/TDX 分析 | `docs/TEE_SGX_TDX_BOT_ANALYSIS.md` | SGX+TDX 双重证明分析 |
| Discord 群管设计 | `docs/NEXUS_DISCORD_GROUP_MANAGEMENT.md` | Discord 平台适配设计 |
| 节点奖励设计 | `docs/NEXUS_NODE_REWARD_DESIGN.md` | 订阅费+通胀混合奖励模型 |
| 社区积分治理 | `docs/COMMUNITY_POINTS_GOVERNANCE.md` | CRM + 积分系统设计 |
| Agent 威胁模型 | `docs/AGENT_THREAT_MODEL.md` | 安全威胁分析与对策 |
| AI 自主运行 | `docs/AI_AUTONOMOUS_OPERATION.md` | AI 驱动的自动化运维方案 |
| 前端技术栈 | `docs/TECH_STACK.md` | 前端技术选型 |

## 许可证

MIT-0
