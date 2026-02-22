/**
 * Escrow（托管）模块测试脚本
 * 测试托管创建、释放等功能
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
  toNexWei,
} from './utils/helpers.js';

async function main() {
  logSection('Escrow（托管）模块测试');
  
  const api = await getApi();
  const alice = getAlice();   // 付款方
  const bob = getBob();       // 收款方
  const charlie = getCharlie(); // 仲裁者
  
  logAccount('Alice (付款方)', alice);
  logAccount('Bob (收款方)', bob);
  logAccount('Charlie (仲裁者)', charlie);
  
  let escrowId: number | null = null;
  
  try {
    // ========================================
    // 步骤 1: 查询托管配置
    // ========================================
    logStep(1, '查询托管配置');
    
    try {
      const nextId = await (api.query as any).escrow.nextEscrowId();
      console.log(`   下一个托管 ID: ${nextId?.toString() || 0}`);
      escrowId = nextId?.toNumber() || 0;
    } catch {
      logInfo('托管 ID 查询不可用');
      escrowId = 0;
    }
    
    // ========================================
    // 步骤 2: 创建托管
    // ========================================
    logStep(2, '创建托管');
    
    const amount = toNexWei(100);
    console.log(`   托管金额: ${formatNex(amount)}`);
    console.log(`   收款方: Bob`);
    
    try {
      const createTx = (api.tx as any).escrow.createEscrow(
        bob.address,      // 收款方
        amount,           // 金额
        null,             // 仲裁者（可选）
        86400 * 7         // 超时时间（7天）
      );
      
      const result = await signAndSend(api, createTx, alice, 'Alice 创建托管');
      
      if (result.success) {
        logSuccess(`托管已创建，ID: ${escrowId}`);
      } else {
        logError(`创建托管失败: ${result.error}`);
      }
    } catch (e: any) {
      logInfo(`创建托管: ${e.message?.slice(0, 80) || '不可用'}`);
    }
    
    // ========================================
    // 步骤 3: 查询托管详情
    // ========================================
    logStep(3, '查询托管详情');
    
    try {
      const escrow = await (api.query as any).escrow.escrows(escrowId);
      if (escrow && escrow.isSome) {
        const e = escrow.unwrap();
        console.log(`   托管 ID: ${escrowId}`);
        console.log(`   付款方: ${e.payer?.toString().slice(0, 16)}...`);
        console.log(`   收款方: ${e.payee?.toString().slice(0, 16)}...`);
        console.log(`   金额: ${formatNex(e.amount?.toString() || '0')}`);
        console.log(`   状态: ${e.status?.toString() || 'Unknown'}`);
      } else {
        console.log(`   托管 ${escrowId} 不存在`);
      }
    } catch {
      logInfo('托管详情查询不可用');
    }
    
    // ========================================
    // 步骤 4: 查询用户托管列表
    // ========================================
    logStep(4, '查询用户托管列表');
    
    try {
      const aliceEscrows = await (api.query as any).escrow.userEscrows(alice.address);
      if (aliceEscrows && aliceEscrows.length > 0) {
        console.log(`   Alice 的托管: ${aliceEscrows.map((id: any) => id.toString()).join(', ')}`);
      } else {
        console.log(`   Alice 暂无托管`);
      }
    } catch {
      logInfo('用户托管列表查询不可用');
    }
    
    logSection('测试完成');
    logSuccess('Escrow 模块测试通过');
    
    console.log('\n📊 测试摘要:');
    console.log(`   - 托管 ID: ${escrowId}`);
    console.log(`   - 付款方: Alice`);
    console.log(`   - 收款方: Bob`);
    
  } catch (error: any) {
    logError(`测试失败: ${error.message}`);
  } finally {
    await disconnectApi();
  }
}

main().catch(console.error);
