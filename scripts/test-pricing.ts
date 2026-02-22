/**
 * Pricing 模块测试脚本
 * 测试 NEX 价格查询功能
 */

import { getApi, disconnectApi } from './utils/api.js';
import { logSection, logStep, logSuccess, logError, logQuery, formatNex, formatUsdt } from './utils/helpers.js';

async function main() {
  logSection('Pricing 模块测试');
  
  const api = await getApi();
  
  try {
    // ========================================
    // 步骤 1: 查询默认价格
    // ========================================
    logStep(1, '查询默认价格');
    
    const defaultPrice = await (api.query as any).tradingPricing.defaultPrice();
    const defaultPriceNum = defaultPrice.toNumber();
    console.log(`   默认价格: ${defaultPriceNum} (${formatUsdt(defaultPriceNum)})`);
    
    // ========================================
    // 步骤 2: 查询冷启动状态
    // ========================================
    logStep(2, '查询冷启动状态');
    
    const coldStartExited = await (api.query as any).tradingPricing.coldStartExited();
    console.log(`   冷启动已退出: ${coldStartExited.isTrue ? '是' : '否'}`);
    
    const coldStartThreshold = await (api.query as any).tradingPricing.coldStartThreshold();
    console.log(`   冷启动阈值: ${formatNex(coldStartThreshold.toString())}`);
    
    // ========================================
    // 步骤 3: 查询 OTC 价格聚合数据
    // ========================================
    logStep(3, '查询 OTC 价格聚合数据');
    
    const otcAggregate = await (api.query as any).tradingPricing.otcPriceAggregate();
    const otcData = {
      totalNex: otcAggregate.totalNex.toString(),
      totalUsdt: otcAggregate.totalUsdt.toString(),
      orderCount: otcAggregate.orderCount.toNumber(),
    };
    console.log(`   OTC 总 NEX: ${formatNex(otcData.totalNex)}`);
    console.log(`   OTC 总 USDT: ${formatUsdt(Number(otcData.totalUsdt))}`);
    console.log(`   OTC 订单数: ${otcData.orderCount}`);
    
    if (BigInt(otcData.totalNex) > 0n) {
      const otcAvgPrice = (BigInt(otcData.totalUsdt) * BigInt(1e12)) / BigInt(otcData.totalNex);
      console.log(`   OTC 均价: ${formatUsdt(Number(otcAvgPrice))}`);
    }
    
    // ========================================
    // 步骤 4: 查询 Bridge 价格聚合数据
    // ========================================
    logStep(4, '查询 Bridge 价格聚合数据');
    
    const bridgeAggregate = await (api.query as any).tradingPricing.bridgePriceAggregate();
    const bridgeData = {
      totalNex: bridgeAggregate.totalNex.toString(),
      totalUsdt: bridgeAggregate.totalUsdt.toString(),
      orderCount: bridgeAggregate.orderCount.toNumber(),
    };
    console.log(`   Bridge 总 NEX: ${formatNex(bridgeData.totalNex)}`);
    console.log(`   Bridge 总 USDT: ${formatUsdt(Number(bridgeData.totalUsdt))}`);
    console.log(`   Bridge 兑换数: ${bridgeData.orderCount}`);
    
    if (BigInt(bridgeData.totalNex) > 0n) {
      const bridgeAvgPrice = (BigInt(bridgeData.totalUsdt) * BigInt(1e12)) / BigInt(bridgeData.totalNex);
      console.log(`   Bridge 均价: ${formatUsdt(Number(bridgeAvgPrice))}`);
    }
    
    // ========================================
    // 步骤 5: 计算当前市场价格
    // ========================================
    logStep(5, '计算当前市场价格');
    
    let currentPrice: number;
    
    if (!coldStartExited.isTrue) {
      currentPrice = defaultPriceNum;
      console.log(`   状态: 冷启动阶段，使用默认价格`);
    } else {
      const totalNex = BigInt(otcData.totalNex) + BigInt(bridgeData.totalNex);
      const totalUsdt = BigInt(otcData.totalUsdt) + BigInt(bridgeData.totalUsdt);
      
      if (totalNex > 0n) {
        currentPrice = Number((totalUsdt * BigInt(1e12)) / totalNex);
      } else {
        currentPrice = defaultPriceNum;
      }
      console.log(`   状态: 正常市场定价`);
    }
    
    console.log(`\n   💰 当前 NEX 价格: ${formatUsdt(currentPrice)}`);
    console.log(`   💰 原始值: ${currentPrice} (精度 10^6)`);
    
    // ========================================
    // 步骤 6: 查询 CNY/USDT 汇率
    // ========================================
    logStep(6, '查询 CNY/USDT 汇率');
    
    const cnyUsdtRate = await (api.query as any).tradingPricing.cnyUsdtRate();
    const cnyRate = cnyUsdtRate.cnyRate.toNumber();
    const updatedAt = cnyUsdtRate.updatedAt.toNumber();
    
    if (cnyRate > 0) {
      console.log(`   CNY/USDT 汇率: ¥${(cnyRate / 1e6).toFixed(4)}`);
      console.log(`   更新时间: ${new Date(updatedAt * 1000).toLocaleString()}`);
    } else {
      console.log(`   CNY/USDT 汇率: 未设置（使用默认 7.2）`);
    }
    
    // ========================================
    // 总结
    // ========================================
    logSection('测试完成');
    logSuccess('Pricing 模块查询测试通过');
    
    console.log('\n📊 价格摘要:');
    console.log(`   - 默认价格: ${formatUsdt(defaultPriceNum)}`);
    console.log(`   - 当前价格: ${formatUsdt(currentPrice)}`);
    console.log(`   - 冷启动状态: ${coldStartExited.isTrue ? '已退出' : '进行中'}`);
    console.log(`   - OTC 订单数: ${otcData.orderCount}`);
    console.log(`   - Bridge 兑换数: ${bridgeData.orderCount}`);
    
  } catch (error: any) {
    logError(`测试失败: ${error.message}`);
    console.error(error);
  } finally {
    await disconnectApi();
  }
}

main().catch(console.error);
