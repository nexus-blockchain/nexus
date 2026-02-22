# Nexus 链上功能测试脚本规划

本文档规划了所有可以通过脚本测试的链上功能模块。

## 测试环境要求

- 本地开发节点运行中 (`./target/release/nexus-node --dev`)
- 测试账户有足够余额（Alice, Bob, Charlie 等开发账户）
- polkadot-js API 或 subxt 客户端

---

## 1. Trading 模块测试

### 1.1 Maker（做市商）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 锁定押金 | `tradingMaker.lockDeposit()` | 锁定 10,000 NEX 押金 |
| 提交申请信息 | `tradingMaker.submitInfo(realName, idCard, birthday, tronAddr, wechatId)` | 提交做市商资料 |
| 审批通过 | `tradingMaker.approveMaker(makerId)` | Root 权限审批 |
| 审批拒绝 | `tradingMaker.rejectMaker(makerId)` | Root 权限拒绝 |
| 申请提现 | `tradingMaker.requestWithdrawal(amount)` | 申请提取押金 |
| 执行提现 | `tradingMaker.executeWithdrawal()` | 冷却期后执行 |
| 取消提现 | `tradingMaker.cancelWithdrawal()` | 取消提现申请 |
| 补充押金 | `tradingMaker.replenishDeposit()` | 补充押金到最低要求 |
| 申诉惩罚 | `tradingMaker.appealPenalty(penaltyId, evidenceCid)` | 对惩罚提起申诉 |

**查询接口**:
- `tradingMaker.makerApplications(makerId)` - 做市商申请信息
- `tradingMaker.accountToMakerId(account)` - 账户对应的做市商ID
- `tradingMaker.nextMakerId()` - 下一个做市商ID

### 1.2 OTC（场外交易）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 创建订单 | `tradingOtc.createOrder(makerId, cosAmount, paymentCommit, contactCommit)` | 买家创建购买订单 |
| 首购订单 | `tradingOtc.createFirstPurchase(makerId, paymentCommit, contactCommit)` | 新用户首购 $10 |
| 标记已付款 | `tradingOtc.markPaid(orderId, tronTxHash?)` | 买家标记已付款 |
| 释放 NEX | `tradingOtc.releaseCos(orderId)` | 做市商确认收款后释放 |
| 取消订单 | `tradingOtc.cancelOrder(orderId)` | 买家取消未付款订单 |
| 发起争议 | `tradingOtc.disputeOrder(orderId)` | 买家/做市商发起争议 |

**查询接口**:
- `tradingOtc.orders(orderId)` - 订单详情
- `tradingOtc.buyerOrders(account)` - 买家订单列表
- `tradingOtc.makerOrders(makerId)` - 做市商订单列表
- `tradingOtc.hasFirstPurchased(account)` - 是否已首购

### 1.3 Swap（兑换）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 创建兑换 | `tradingSwap.makerSwap(makerId, cosAmount, usdtAddress)` | 用户发起 NEX→USDT 兑换 |
| 提交交易哈希 | `tradingSwap.markSwapComplete(swapId, trc20TxHash)` | 做市商提交 USDT 转账哈希 |
| 举报做市商 | `tradingSwap.reportSwap(swapId)` | 用户举报未履约 |
| 处理验证超时 | `tradingSwap.handleVerificationTimeout(swapId)` | 触发超时退款 |

**查询接口**:
- `tradingSwap.makerSwaps(swapId)` - 兑换详情
- `tradingSwap.userSwaps(account)` - 用户兑换列表
- `tradingSwap.makerSwapList(makerId)` - 做市商兑换列表

### 1.4 Pricing（定价）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 更新冷启动参数 | `tradingPricing.updateColdStartParams(threshold?, defaultPrice?)` | Root 权限 |
| 重置冷启动 | `tradingPricing.resetColdStart(reason)` | Root 权限紧急重置 |

**查询接口**:
- `tradingPricing.otcPriceAggregate()` - OTC 价格聚合数据
- `tradingPricing.bridgePriceAggregate()` - Bridge 价格聚合数据
- `tradingPricing.defaultPrice()` - 默认价格
- `tradingPricing.coldStartExited()` - 是否退出冷启动
- `tradingPricing.cnyUsdtRate()` - CNY/USDT 汇率

### 1.5 Credit（信用分）模块

**查询接口**:
- `tradingCredit.makerCredits(makerId)` - 做市商信用分

---

## 2. Referral（推荐）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 绑定推荐人 | `referral.bindSponsor(sponsorCode)` | 绑定上线推荐码 |
| 设置推荐码 | `referral.setReferralCode(code)` | 设置自己的推荐码 |

**查询接口**:
- `referral.sponsors(account)` - 上线账户
- `referral.downlines(account)` - 下线列表
- `referral.referralCodes(account)` - 推荐码

---

## 3. Chat（聊天）模块

### 3.1 Contacts（联系人）

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 发送好友请求 | `chatContacts.sendFriendRequest(target, message?)` | 发送好友请求 |
| 接受好友请求 | `chatContacts.acceptFriendRequest(requestId)` | 接受请求 |
| 拒绝好友请求 | `chatContacts.rejectFriendRequest(requestId)` | 拒绝请求 |
| 删除好友 | `chatContacts.removeFriend(friendAccount)` | 删除好友 |

### 3.2 Messages（消息）

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 发送私聊消息 | `chatMessages.sendMessage(recipient, contentCid)` | 发送加密消息 |

### 3.3 Groups（群组）

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 创建群组 | `chatGroups.createGroup(name, description?, avatar?)` | 创建群组 |
| 加入群组 | `chatGroups.joinGroup(groupId)` | 加入群组 |
| 离开群组 | `chatGroups.leaveGroup(groupId)` | 离开群组 |
| 解散群组 | `chatGroups.disbandGroup(groupId)` | 群主解散 |
| 发送群消息 | `chatGroups.sendGroupMessage(groupId, contentCid)` | 发送群消息 |

---

## 4. Escrow（托管）模块

| 测试项 | 说明 |
|--------|------|
| 锁定资金 | 通过 OTC/Swap 订单自动触发 |
| 释放资金 | 通过订单完成自动触发 |
| 退款 | 通过订单取消/超时自动触发 |

**查询接口**:
- `escrow.escrows(escrowId)` - 托管详情
- `escrow.accountEscrows(account)` - 账户托管列表

---

## 5. Arbitration（仲裁）模块

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| 发起仲裁 | `arbitration.initiateArbitration(caseType, targetId, evidenceCid)` | 发起仲裁案件 |
| 提交证据 | `arbitration.submitEvidence(caseId, evidenceCid)` | 提交仲裁证据 |
| 仲裁裁决 | `arbitration.resolveArbitration(caseId, decision)` | 仲裁员裁决 |

**查询接口**:
- `arbitration.cases(caseId)` - 仲裁案件详情

---

## 6. Storage（存储）模块

### 8.1 Storage Service

| 测试项 | Extrinsic | 说明 |
|--------|-----------|------|
| PIN CID | `storageService.pinCid(cid, size)` | PIN 文件到 IPFS |
| Unpin CID | `storageService.unpinCid(cid)` | Unpin 文件 |

---

## 测试脚本目录结构

```
scripts/
├── TEST_PLAN.md              # 本文档
├── test-maker.ts             # 做市商功能测试
├── test-otc.ts               # OTC 交易测试
├── test-swap.ts              # 兑换功能测试
├── test-pricing.ts           # 定价模块测试
├── test-referral.ts          # 推荐系统测试
├── test-chat.ts              # 聊天功能测试
├── test-escrow.ts            # 托管功能测试
├── test-arbitration.ts       # 仲裁功能测试
├── test-storage.ts           # 存储功能测试
├── utils/
│   ├── api.ts                # API 连接工具
│   ├── accounts.ts           # 测试账户管理
│   └── helpers.ts            # 通用辅助函数
└── run-all-tests.sh          # 运行所有测试
```

---

## 测试优先级

### P0 - 核心交易流程（必须测试）
1. **做市商申请流程**: 锁定押金 → 提交信息 → 审批
2. **OTC 完整流程**: 创建订单 → 付款 → 释放
3. **Swap 完整流程**: 创建兑换 → 提交哈希 → 验证完成
4. **价格查询**: 获取当前 NEX 价格

### P1 - 重要功能
5. **推荐系统**: 绑定推荐人 → 查询上下线
6. **聊天功能**: 好友请求 → 发送消息
7. **托管查询**: 查询托管状态

### P2 - 次要功能
8. **仲裁流程**: 发起仲裁 → 提交证据 → 裁决

---

## 运行测试

```bash
# 安装依赖
cd scripts
npm install

# 运行单个测试
npx ts-node test-maker.ts

# 运行所有测试
./run-all-tests.sh
```
