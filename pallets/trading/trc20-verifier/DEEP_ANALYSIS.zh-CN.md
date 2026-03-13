# pallet-trading-trc20-verifier 深度分析

日期：2026-03-13  
范围：

- `trading/trc20-verifier/src/lib.rs`
- `trading/trc20-verifier/README.md`
- `trading/nex-market/src/lib.rs`

测试结果：

- `cargo test`：**175/175 通过**

---

## 总结

这个模块现在是**可用原型**，但还不是**可安全上生产**的验证器。

最大问题集中在：

- 付款归属歧义
- split payment / 补付的 `tx_hash` 证明不完整
- 对恶意端点过度信任
- 并行/共识实现与文档不一致

---

## 一、按角色 / 业务流看问题

### 1）买家视角：正常全额付款

#### 风险：被误判没付 / 少付

- 全局限流是单 key：`check_rate_limit()` 使用 `ocw_rate_limit_last_req` 全局拦截，和交易无关。高并发下别的订单会影响当前订单。  
  位置：`trading/trc20-verifier/src/lib.rs:1212-1231`
- 锁粒度是 `from:to`，**不含 trade_id / amount / min_timestamp**。同一买家和卖家多笔并行订单会互相卡住。  
  位置：`trading/trc20-verifier/src/lib.rs:2313-2325`
- `parallel_mode=true` 默认开启，但“并行竞速”实际上是**顺序 wait**，不是真正最快返回。  
  位置：`trading/trc20-verifier/src/lib.rs:1601-1698`  
  README 却写“取最快成功响应”：`trading/trc20-verifier/README.md:299-302`

#### 必须补

- 锁 key 改成至少包含 `trade_id` 或 `(from,to,amount,min_timestamp)`
- 限流改成**按端点/按订单**
- 并行改成真正 fan-out + first-success 收敛，否则默认应关闭

---

### 2）买家视角：少付后补付

#### 风险：补付证明丢失，后续 tx_hash 可被复用

- verifier 支持 `matched_transfers` 多笔明细，但 nex-market 最终只上链一个 `tx_hash`  
  位置：`trading/nex-market/src/lib.rs:1874-1953`, `2168-2251`
- `submit_underpaid_update` **不带 tx_hash**，只更新累计金额  
  位置：`trading/nex-market/src/lib.rs:2263-2305`

#### 这意味着

- 一笔订单如果由 2~3 个 TRON 转账拼出来，链上只登记了其中 1 个 hash，**其余 hash 可在别的订单再次被用作支付证明**
- 这是当前设计里最危险的业务漏洞之一

#### 必须补

- 所有结算入口都支持 `Vec<TxHash>`
- 原子登记**全部** proof
- `submit_underpaid_update` 不能只传金额，必须传**新增 tx_hash 集合**或完整 proof 集合

---

### 3）卖家视角：收款后自动放币

#### 风险：被旧交易 / 错地址交易 / 伪造响应骗放币

这是最严重的一组。

#### 严重漏洞 A：代码根本没有在响应里校验 `to_address`

`verify_trc20_by_transfer()` 收到了 `to_address`，但解析时只检查：

- `from`
- `token_info.address`

没有检查 `to`。

位置：

- `trading/trc20-verifier/src/lib.rs:2278-2383`
- `parse_trc20_transfer_list_paged()`：`trading/trc20-verifier/src/lib.rs:2591-2605`
- `extract_normalized_transfers()`：`trading/trc20-verifier/src/lib.rs:2086-2124`

#### 后果

只要端点返回“from 正确、contract 正确”，哪怕那笔钱是打给别人的，也可能被当成有效付款。

#### 严重漏洞 B：代码没有在响应里校验 `block_timestamp >= min_timestamp`

`min_timestamp` 只放进 URL 查询参数，但解析结果时**没有二次校验**。

位置：构造 URL 在 `trading/trc20-verifier/src/lib.rs:2347-2356`，解析时没有对应检查 `trading/trc20-verifier/src/lib.rs:2532-2668`

#### 后果

恶意 / 异常端点可以把**历史旧转账**塞回来，骗过当前订单验证。

#### 严重漏洞 C：允许“没有 tx_hash”的金额进入累计

`parse_trc20_transfer_list_paged()` 里，即使 `transaction_id` 缺失，也会把金额计入 `total_matched_amount`。  
位置：`trading/trc20-verifier/src/lib.rs:2629-2638`

最后只在“整体 `found=true` 但主 `tx_hash` 为空”时降级失败：`trading/trc20-verifier/src/lib.rs:2427-2434`

这意味着：

- 如果 1 笔真实小额转账有 hash
- 再混入几笔**没有 hash 的伪金额**
- 结果仍可能 `Exact/Overpaid`

#### 严重漏洞 D：缺少 timestamp 也能过

如果 `block_timestamp` 不存在，代码直接跳过确认数检查。  
位置：`trading/trc20-verifier/src/lib.rs:2607-2623`

#### 必须补

- 响应侧强校验：`to == expected_to`
- 强校验：`block_timestamp >= min_timestamp && block_timestamp <= now`
- 强校验：`transaction_id` 必填，且格式 / 长度必须符合 TRON tx hash
- 强校验：`block_timestamp` 必填
- 不满足任一条件，**该条 transfer 直接丢弃**

---

### 4）OCW / sidecar / 验证节点视角

#### 风险：被恶意端点拖死、骗空共识、伪竞速

- `response.body().collect::<Vec<u8>>()` 没有 body 大小上限，恶意端点可做内存 / CPU DoS  
  位置：`trading/trc20-verifier/src/lib.rs:1660`, `1759`, `1879`
- 共识层把很多“坏响应”当成空列表参与投票：
  - `success != true` → `Ok(Vec::new())`
  - `data` 缺失 → `Ok(Vec::new())`
  - 解析失败在 fetch 层又 `unwrap_or_default()`
  位置：`trading/trc20-verifier/src/lib.rs:2059-2076`, `1488-1490`

#### 后果

2 个返回空数据 / 失败 JSON 的端点，可能“共识”压过 1 个真正返回付款记录的端点，造成**假阴性**。

---

### 5）管理员 / 运维视角

很多配置项现在是“看起来有，实际价值很低”。

- `priority_boosts`、health score 只在串行模式使用；默认 `parallel_mode=true` 时基本没用  
  `get_sorted_endpoints()`：`trading/trc20-verifier/src/lib.rs:1100-1120`  
  串行使用：`trading/trc20-verifier/src/lib.rs:1800-1808`
- URL 缓存按**完整 URL**做 key，而 nex-market 每次传入的 `min_timestamp = now - 24h / 48h` 都在变，命中率会非常低  
  缓存：`trading/trc20-verifier/src/lib.rs:1241-1266`  
  调用：`trading/nex-market/src/lib.rs:4066-4068`, `4166-4167`, `4216-4217`
- README 说“只缓存合法 JSON”，但代码只是判断首字节是不是 `{` 或 `[`  
  代码：`trading/trc20-verifier/src/lib.rs:1537-1540`, `1258-1266`  
  文档：`trading/trc20-verifier/README.md:310-312`

---

## 二、必须马上补的功能

按优先级排序：

### P0

1. **响应侧强校验**
   - `to`
   - `min_timestamp`
   - `transaction_id`
   - `block_timestamp`
   - `tx_hash` 格式

2. **多 tx_hash 证明模型**
   - verifier 返回 `Vec<TxHash>`
   - chain 侧结算登记所有 hash
   - 补付更新必须带新增 hash

3. **订单唯一绑定机制**

   现在按 `(from,to,amount,time window)` 匹配，天然有歧义。必须新增至少一种：

   - 每单唯一收款地址
   - 每单唯一随机尾差金额
   - 每单唯一支付 reference（如果通道支持）

4. **真正的并行竞速 / 否则默认关闭**

   现在这个“竞速”实现名不副实。

5. **共识层重写**
   - 只让“成功且结构合法且字段完整”的响应参与共识
   - 共识对象必须包含 `to / min_timestamp / confirmations`
   - 禁止默认 `single_source_fallback=true`

### P1

6. **按订单 / 端点限流**
7. **响应大小上限**
8. **锁粒度细化 + 随机 token**
9. **审计日志记录全量 proof，而不是单 hash**
10. **区分“未付款”和“验证系统失败”两类 metrics**

---

## 三、冗余 / 误导性功能

1. **单个 `tx_hash` 字段**

   现在已有 `matched_transfers`，再保留单个主 `tx_hash` 很容易误导上层“只登记一个 hash 就够了”。

2. **offchain 本地 `used tx_hash` 注册表**

   对跨节点一致性几乎没保证；真正可靠的是链上 `UsedTxHashes`。现在这个本地表更像“局部优化 / 假安全”。

3. **priority_boost / health score**

   在默认并行模式下几乎不生效，复杂度大于收益。

4. **URL 级缓存**

   在当前 nex-market 集成方式里命中率很低，且还可能缓存业务失败响应。

5. **`VerificationError::TxHashAlreadyUsed`**

   这个 error variant 在 verifier crate 里基本没被实际返回，API 有点虚胖。

---

## 四、明确的代码 BUG

1. **分页提前结束发生在已用 tx 过滤之前**

   位置：`trading/trc20-verifier/src/lib.rs:2368-2383` 先决定 break，`2394-2424` 才过滤 used tx。  
   可能漏掉后页里真正可用的付款。

2. **`merge_transfer_results()` 不会在后续找到匹配后清掉旧 error**

   位置：`trading/trc20-verifier/src/lib.rs:2463-2508`  
   可能出现 `found=true` 但还带着上一页 `"No matching transfer found"` 的脏错误。

3. **EndpointHealth 默认 score=0，但 `calculate_score()` 的“默认中等分数”是 50**

   - struct derive `Default`：`trading/trc20-verifier/src/lib.rs:217-230`
   - `calculate_score()`：`trading/trc20-verifier/src/lib.rs:238-241`
   - `get_endpoint_health()` 直接 `unwrap_or_default()`：`trading/trc20-verifier/src/lib.rs:300-306`

   会导致新端点初始排序和文档语义不一致。

4. **锁 token 用毫秒时间戳，理论上可碰撞**

   位置：`trading/trc20-verifier/src/lib.rs:685-710`  
   同毫秒重入时，旧持有者可能误释放新锁。

---

## 五、最终判断

一句话：

> 当前 `pallet-trading-trc20-verifier` 更像“可工作的原型”，不是“可抗攻击、可支撑真实多订单 / 补付业务”的生产级验证器。

如果后续继续推进，建议下一步直接做三件事：

1. 按严重级别排序修复清单（P0 / P1 / P2）
2. 明确数据结构与函数签名修改方案
3. 输出可直接落地的 patch 设计草案

