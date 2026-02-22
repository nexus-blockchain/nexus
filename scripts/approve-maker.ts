/**
 * 做市商审批脚本
 * 用于审批待审核的做市商申请
 * 
 * 使用方法:
 *   npx tsx approve-maker.ts [makerId]
 *   
 * 示例:
 *   npx tsx approve-maker.ts 0      # 审批 ID 为 0 的做市商
 *   npx tsx approve-maker.ts        # 列出所有待审批的做市商
 */

import { getApi, disconnectApi } from './utils/api.js';
import { getAlice, logAccount } from './utils/accounts.js';
import { 
  signAndSend, 
  logSection, 
  logStep, 
  logSuccess, 
  logError, 
  logInfo,
  formatNex,
} from './utils/helpers.js';

// 做市商状态枚举
const ApplicationStatus = {
  DepositLocked: 'DepositLocked',
  PendingReview: 'PendingReview',
  Active: 'Active',
  Rejected: 'Rejected',
  Cancelled: 'Cancelled',
  Expired: 'Expired',
};

async function main() {
  const args = process.argv.slice(2);
  const targetMakerId = args[0] ? parseInt(args[0]) : null;
  
  logSection('做市商审批工具');
  
  const api = await getApi();
  const alice = getAlice(); // Root 权限账户
  
  logAccount('Alice (Root)', alice);
  
  try {
    // ========================================
    // 步骤 1: 查询所有做市商
    // ========================================
    logStep(1, '查询做市商列表');
    
    const nextMakerId = await (api.query as any).tradingMaker.nextMakerId();
    const totalMakers = nextMakerId.toNumber();
    console.log(`   总做市商数量: ${totalMakers}`);
    
    if (totalMakers === 0) {
      logInfo('暂无做市商申请');
      return;
    }
    
    // 收集所有做市商信息
    const makers: any[] = [];
    for (let i = 0; i < totalMakers; i++) {
      const makerApp = await (api.query as any).tradingMaker.makerApplications(i);
      if (makerApp.isSome) {
        const app = makerApp.unwrap();
        makers.push({
          id: i,
          owner: app.owner.toString(),
          status: app.status.toString(),
          deposit: app.deposit.toString(),
          servicePaused: app.servicePaused,
        });
      }
    }
    
    // 显示做市商列表
    console.log('\n   做市商列表:');
    console.log('   ' + '-'.repeat(80));
    console.log(`   ${'ID'.padEnd(6)} ${'状态'.padEnd(16)} ${'押金'.padEnd(20)} ${'账户'}`);
    console.log('   ' + '-'.repeat(80));
    
    for (const maker of makers) {
      const statusDisplay = maker.status.padEnd(14);
      const depositDisplay = formatNex(maker.deposit).padEnd(18);
      const ownerShort = `${maker.owner.slice(0, 12)}...${maker.owner.slice(-6)}`;
      console.log(`   ${String(maker.id).padEnd(6)} ${statusDisplay} ${depositDisplay} ${ownerShort}`);
    }
    console.log('   ' + '-'.repeat(80));
    
    // 筛选待审批的做市商
    const pendingMakers = makers.filter(m => 
      m.status === ApplicationStatus.DepositLocked || 
      m.status === ApplicationStatus.PendingReview
    );
    
    console.log(`\n   待审批数量: ${pendingMakers.length}`);
    
    // ========================================
    // 步骤 2: 审批指定做市商或全部待审批
    // ========================================
    if (targetMakerId !== null) {
      // 审批指定 ID
      logStep(2, `审批做市商 ID: ${targetMakerId}`);
      
      const maker = makers.find(m => m.id === targetMakerId);
      if (!maker) {
        logError(`做市商 ID ${targetMakerId} 不存在`);
        return;
      }
      
      console.log(`   当前状态: ${maker.status}`);
      
      if (maker.status === ApplicationStatus.Active) {
        logInfo('该做市商已激活，无需审批');
        return;
      }
      
      if (maker.status === ApplicationStatus.Rejected || 
          maker.status === ApplicationStatus.Cancelled ||
          maker.status === ApplicationStatus.Expired) {
        logError(`该做市商状态为 ${maker.status}，无法审批`);
        return;
      }
      
      // 执行审批
      await approveMaker(api, alice, targetMakerId);
      
    } else if (pendingMakers.length > 0) {
      // 列出待审批的做市商，询问是否全部审批
      logStep(2, '待审批做市商');
      
      console.log('\n   待审批列表:');
      for (const maker of pendingMakers) {
        console.log(`   - ID: ${maker.id}, 状态: ${maker.status}, 账户: ${maker.owner.slice(0, 16)}...`);
      }
      
      console.log('\n   提示: 使用 "npx tsx approve-maker.ts <makerId>" 审批指定做市商');
      console.log('   示例: npx tsx approve-maker.ts 0');
      
    } else {
      logInfo('没有待审批的做市商');
    }
    
    // ========================================
    // 步骤 3: 显示最终状态
    // ========================================
    if (targetMakerId !== null) {
      logStep(3, '查询审批后状态');
      
      const finalApp = await (api.query as any).tradingMaker.makerApplications(targetMakerId);
      if (finalApp.isSome) {
        const app = finalApp.unwrap();
        console.log(`   做市商 ID: ${targetMakerId}`);
        console.log(`   最终状态: ${app.status.toString()}`);
        console.log(`   押金: ${formatNex(app.deposit.toString())}`);
        console.log(`   服务暂停: ${app.servicePaused ? '是' : '否'}`);
        
        if (app.status.isActive) {
          logSuccess('做市商已激活，可以开始接单！');
        }
      }
    }
    
    logSection('完成');
    
  } catch (error: any) {
    logError(`执行失败: ${error.message}`);
    console.error(error);
  } finally {
    await disconnectApi();
  }
}

async function approveMaker(api: any, alice: any, makerId: number): Promise<boolean> {
  console.log(`\n   正在审批做市商 ID: ${makerId}...`);
  
  // 使用 sudo 调用 approveMaker
  const approveTx = (api.tx as any).tradingMaker.approveMaker(makerId);
  const sudoTx = api.tx.sudo.sudo(approveTx);
  
  const result = await signAndSend(api, sudoTx, alice, `审批做市商 ${makerId}`);
  
  if (result.success) {
    logSuccess(`做市商 ${makerId} 审批成功！`);
    return true;
  } else {
    logError(`做市商 ${makerId} 审批失败: ${result.error}`);
    return false;
  }
}

main().catch(console.error);
