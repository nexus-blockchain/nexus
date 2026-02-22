/**
 * OTC（场外交易）模块测试脚本
 * 测试 OTC 订单创建、付款、释放等完整流程
 */

import { getApi, disconnectApi } from './utils/api.js';
import { getAlice, getBob, getCharlie, logAccount } from './utils/accounts.js';
import { 
  signAndSend, 
  logSection, 
  logStep, 
  logSuccess, 
  logError, 
  logInfo,
  formatNex,
  formatUsdt,
  toNexWei,
  sleep
} from './utils/helpers.js';
import { blake2AsHex } from '@polkadot/util-crypto';

async function main() {
  logSection('OTC（场外交易）模块测试');
  
  const api = await getApi();
  const alice = getAlice();   // Root 权限
  const bob = getBob();       // 做市商
  const charlie = getCharlie(); // 买家
  
  logAccount('Alice (Root)', alice);
  logAccount('Bob (做市商)', bob);
  logAccount('Charlie (买家)', charlie);
  
  let makerId: number | null = null;
  let orderId: number | null = null;
  
  try {
    // ========================================
    // 步骤 1: 确保 Bob 是激活的做市商
    // ========================================
    logStep(1, '确保 Bob 是激活的做市商');
    
    const bobMakerId = await (api.query as any).tradingMaker.accountToMaker(bob.address);
    if (bobMakerId.isSome) {
      makerId = bobMakerId.unwrap().toNumber();
      const makerApp = await (api.query as any).tradingMaker.makerApplications(makerId);
      if (makerApp.isSome && makerApp.unwrap().status.isActive) {
        logSuccess(`Bob 是激活的做市商，ID: ${makerId}`);
      } else {
        logError('Bob 不是激活的做市商，请先运行 test-maker.ts');
        return;
      }
    } else {
      logError('Bob 不是做市商，请先运行 test-maker.ts');
      return;
    }
    
    // ========================================
    // 步骤 2: 查询 Charlie 是否已首购
    // ========================================
    logStep(2, '查询 Charlie 是否已首购');
    
    const hasFirstPurchased = await (api.query as any).tradingOtc.hasFirstPurchased(charlie.address);
    console.log(`   Charlie 已首购: ${hasFirstPurchased.isTrue ? '是' : '否'}`);
    
    // ========================================
    // 步骤 3: 创建 OTC 订单
    // ========================================
    logStep(3, '创建 OTC 订单');
    
    // 生成承诺哈希
    const paymentCommit = blake2AsHex(`payment:${charlie.address}:${Date.now()}`);
    const contactCommit = blake2AsHex(`contact:wechat_charlie:${Date.now()}`);
    
    console.log(`   支付承诺: ${paymentCommit.slice(0, 20)}...`);
    console.log(`   联系承诺: ${contactCommit.slice(0, 20)}...`);
    
    // 获取当前订单 ID
    const nextOrderId = await (api.query as any).tradingOtc.nextOrderId();
    orderId = nextOrderId.toNumber();
    console.log(`   预期订单 ID: ${orderId}`);
    
    // 创建订单（购买 1000 NEX）
    const nexAmount = toNexWei(1000);
    const createOrderTx = (api.tx as any).tradingOtc.createOrder(
      makerId,
      nexAmount,
      paymentCommit,
      contactCommit
    );
    
    const createResult = await signAndSend(api, createOrderTx, charlie, 'Charlie 创建 OTC 订单');
    
    if (!createResult.success) {
      logError(`创建订单失败: ${createResult.error}`);
      return;
    }
    
    logSuccess(`订单已创建，ID: ${orderId}`);
    
    // ========================================
    // 步骤 4: 查询订单详情
    // ========================================
    logStep(4, '查询订单详情');
    
    const order = await (api.query as any).tradingOtc.orders(orderId);
    if (order.isSome) {
      const o = order.unwrap();
      console.log(`   订单 ID: ${orderId}`);
      console.log(`   做市商 ID: ${o.makerId.toNumber()}`);
      console.log(`   买家: ${o.taker.toString().slice(0, 16)}...`);
      console.log(`   NEX 数量: ${formatNex(o.qty.toString())}`);
      console.log(`   USDT 金额: ${formatUsdt(o.amount.toNumber())}`);
      console.log(`   状态: ${o.state.toString()}`);
      console.log(`   首购订单: ${o.isFirstPurchase.isTrue ? '是' : '否'}`);
    }
    
    // ========================================
    // 步骤 5: 买家标记已付款
    // ========================================
    logStep(5, '买家标记已付款');
    
    // 模拟 TRON 交易哈希
    const tronTxHash = `${Date.now().toString(16)}abcdef1234567890`;
    console.log(`   TRON 交易哈希: ${tronTxHash}`);
    
    const markPaidTx = (api.tx as any).tradingOtc.markPaid(orderId, tronTxHash);
    const markPaidResult = await signAndSend(api, markPaidTx, charlie, 'Charlie 标记已付款');
    
    if (!markPaidResult.success) {
      logError(`标记付款失败: ${markPaidResult.error}`);
      return;
    }
    
    logSuccess('已标记付款');
    
    // 查询更新后的状态
    const orderAfterPaid = await (api.query as any).tradingOtc.orders(orderId);
    if (orderAfterPaid.isSome) {
      console.log(`   新状态: ${orderAfterPaid.unwrap().state.toString()}`);
    }
    
    // ========================================
    // 步骤 6: 做市商释放 NEX
    // ========================================
    logStep(6, '做市商释放 NEX');
    
    const releaseTx = (api.tx as any).tradingOtc.releaseNex(orderId);
    const releaseResult = await signAndSend(api, releaseTx, bob, 'Bob 释放 NEX');
    
    if (!releaseResult.success) {
      logError(`释放失败: ${releaseResult.error}`);
      return;
    }
    
    logSuccess('NEX 已释放给买家');
    
    // 查询最终状态
    const orderFinal = await (api.query as any).tradingOtc.orders(orderId);
    if (orderFinal.isSome) {
      console.log(`   最终状态: ${orderFinal.unwrap().state.toString()}`);
    }
    
    // ========================================
    // 步骤 7: 查询买家订单列表
    // ========================================
    logStep(7, '查询买家订单列表');
    
    const charlieOrders = await (api.query as any).tradingOtc.buyerOrders(charlie.address);
    if (charlieOrders && charlieOrders.length > 0) {
      const orderIds = charlieOrders.map((id: any) => id.toNumber());
      console.log(`   Charlie 的订单: [${orderIds.join(', ')}]`);
    } else {
      console.log(`   Charlie 暂无订单`);
    }
    
    // ========================================
    // 步骤 8: 查询做市商订单列表
    // ========================================
    logStep(8, '查询做市商订单列表');
    
    const makerOrders = await (api.query as any).tradingOtc.makerOrders(makerId);
    if (makerOrders && makerOrders.length > 0) {
      const orderIds = makerOrders.map((id: any) => id.toNumber());
      console.log(`   做市商 ${makerId} 的订单: [${orderIds.join(', ')}]`);
    } else {
      console.log(`   做市商暂无订单`);
    }
    
    // ========================================
    // 总结
    // ========================================
    logSection('测试完成');
    logSuccess('OTC 模块测试通过');
    
    console.log('\n📊 测试摘要:');
    console.log(`   - 订单 ID: ${orderId}`);
    console.log(`   - 做市商: Bob (ID: ${makerId})`);
    console.log(`   - 买家: Charlie`);
    console.log(`   - 流程: 创建订单 → 标记付款 → 释放 NEX`);
    console.log(`   - 最终状态: Released`);
    
  } catch (error: any) {
    logError(`测试失败: ${error.message}`);
    console.error(error);
  } finally {
    await disconnectApi();
  }
}

main().catch(console.error);
