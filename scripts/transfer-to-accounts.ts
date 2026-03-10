/**
 * 批量转账脚本
 * 从 test-accounts.txt 读取地址，用 ALICE 转账
 * 
 * 使用方法:
 *   npx tsx transfer-to-accounts.ts [amount]
 *   
 * 示例:
 *   npx tsx transfer-to-accounts.ts         # 每个账户转 10 NEX
 *   npx tsx transfer-to-accounts.ts 100     # 每个账户转 100 NEX
 */

import { cryptoWaitReady } from '@polkadot/util-crypto';
import { getApi, disconnectApi } from './utils/api.js';
import { getAlice, getBob, getCharlie, getDave, getEve, logAccount } from './utils/accounts.js';
import { 
  signAndSend, 
  logSection, 
  logStep, 
  logSuccess, 
  logError, 
  formatNex,
  toNexWei,
} from './utils/helpers.js';
import * as fs from 'fs';
import * as path from 'path';

await cryptoWaitReady();

const DEFAULT_AMOUNT = 1000000000; // 默认每个账户转 1000000000 NEX
const FUNDERS = {
  alice: getAlice,
  bob: getBob,
  charlie: getCharlie,
  dave: getDave,
  eve: getEve,
};

function parseAddressesFromFile(filePath: string): string[] {
  const content = fs.readFileSync(filePath, 'utf-8');
  const lines = content.split('\n');
  const addresses: string[] = [];
  
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('地址:')) {
      const address = trimmed.replace('地址:', '').trim();
      if ((address.startsWith('5') || address.startsWith('X')) && address.length >= 47) {
        addresses.push(address);
      }
    }
  }
  
  return addresses;
}

async function main() {
  const args = process.argv.slice(2);
  const amount = args[0] ? parseFloat(args[0]) : DEFAULT_AMOUNT;
  const funderName = (args[1] ?? 'alice').toLowerCase() as keyof typeof FUNDERS;
  
  if (isNaN(amount) || amount <= 0) {
    logError('金额必须是正数');
    return;
  }

  if (!FUNDERS[funderName]) {
    logError(`未知发送账户: ${funderName}`);
    console.log('可用发送账户: alice, bob, charlie, dave, eve');
    return;
  }
  
  logSection('批量转账到测试账户');
  
  // ========================================
  // 步骤 1: 读取账户文件
  // ========================================
  logStep(1, '读取账户文件');
  
  const accountsFile = path.join(process.cwd(), 'test-accounts.txt');
  
  if (!fs.existsSync(accountsFile)) {
    logError(`文件不存在: ${accountsFile}`);
    console.log('请先运行 create-test-accounts.ts 生成账户');
    return;
  }
  
  const addresses = parseAddressesFromFile(accountsFile);
  console.log(`   找到 ${addresses.length} 个地址`);
  
  if (addresses.length === 0) {
    logError('未找到有效地址');
    return;
  }
  
  // 显示前几个地址
  console.log(`   前3个地址:`);
  addresses.slice(0, 3).forEach((addr, i) => {
    console.log(`     ${i + 1}. ${addr.slice(0, 16)}...${addr.slice(-8)}`);
  });
  
  // ========================================
  // 步骤 2: 连接到链
  // ========================================
  logStep(2, '连接到链');
  
  const api = await getApi();
  const funder = FUNDERS[funderName]();
  
  logAccount(`${funderName} (发送方)`, funder);
  
  // 查询发送方余额
  const funderBalance = await api.query.system.account(funder.address);
  console.log(`   ${funderName} 余额: ${formatNex(funderBalance.data.free.toString())}`);
  
  const totalAmount = amount * addresses.length;
  console.log(`   计划转账总额: ${totalAmount} NEX (每账户 ${amount} NEX)`);
  
  // ========================================
  // 步骤 3: 批量转账
  // ========================================
  logStep(3, `向 ${addresses.length} 个账户转账`);
  
  const amountWei = toNexWei(amount);
  let successCount = 0;
  let failCount = 0;
  const utility = (api.tx as any).utility;

  if (utility?.batchAll) {
    const batchTx = utility.batchAll(
      addresses.map((address) => api.tx.balances.transferKeepAlive(address, amountWei))
    );
    const batchResult = await signAndSend(api, batchTx, funder, `批量转账给 ${addresses.length} 个账户`);
    successCount = batchResult.success ? addresses.length : 0;
    failCount = batchResult.success ? 0 : addresses.length;

    if (!batchResult.success) {
      console.log(`   ❌ 批量转账失败: ${batchResult.error}`);
    }
  } else {
    console.log('   ℹ 当前链未启用 utility.batchAll，回退为逐笔转账');

    for (let i = 0; i < addresses.length; i++) {
      const address = addresses[i];
      console.log(`\n   [${i + 1}/${addresses.length}] 转账给: ${address.slice(0, 16)}...`);

      try {
        const transferTx = api.tx.balances.transferKeepAlive(address, amountWei);
        const result = await signAndSend(api, transferTx, funder, `转账给账户 ${i + 1}`);

        if (result.success) {
          successCount++;
          console.log(`   ✅ 成功`);
        } else {
          failCount++;
          console.log(`   ❌ 失败: ${result.error}`);
        }
      } catch (error: any) {
        failCount++;
        console.log(`   ❌ 异常: ${error.message}`);
      }
    }
  }
  
  // ========================================
  // 步骤 4: 验证转账结果
  // ========================================
  logStep(4, '验证转账结果');
  
  console.log(`\n   转账统计:`);
  console.log(`   - 成功: ${successCount}`);
  console.log(`   - 失败: ${failCount}`);
  
  // 抽查几个账户余额
  console.log(`\n   抽查账户余额:`);
  const checkIndices = [0, Math.floor(addresses.length / 2), addresses.length - 1];
  
  for (const idx of checkIndices) {
    if (idx < addresses.length) {
      const balance = await api.query.system.account(addresses[idx]);
      console.log(`   账户 ${idx + 1}: ${formatNex(balance.data.free.toString())}`);
    }
  }
  
  // 查询发送方新余额
  const newFunderBalance = await api.query.system.account(funder.address);
  console.log(`\n   ${funderName} 新余额: ${formatNex(newFunderBalance.data.free.toString())}`);
  
  logSection('完成');
  
  if (failCount === 0) {
    logSuccess(`所有 ${addresses.length} 个账户转账成功！`);
  } else {
    logError(`${failCount} 个账户转账失败，${successCount} 个成功`);
  }
  
  await disconnectApi();
}

main().catch(console.error);
