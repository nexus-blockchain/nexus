/**
 * 创建测试账户脚本
 * 生成20个新账户，保存助记词和地址到文件，并用ALICE转账测试
 * 
 * 使用方法:
 *   npx tsx create-test-accounts.ts [amount]
 *   
 * 示例:
 *   npx tsx create-test-accounts.ts         # 每个账户转 10 NEX
 *   npx tsx create-test-accounts.ts 100     # 每个账户转 100 NEX
 */

import { Keyring } from '@polkadot/keyring';
import { mnemonicGenerate, cryptoWaitReady } from '@polkadot/util-crypto';
import { getApi, disconnectApi } from './utils/api.js';
import { getAlice, getBob, getCharlie, getDave, getEve, logAccount } from './utils/accounts.js';
import { NEXUS_SS58_FORMAT } from './utils/ss58.js';
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

// 初始化 WASM 加密库
await cryptoWaitReady();

const ACCOUNT_COUNT = 20;
const DEFAULT_AMOUNT = 10; // 默认每个账户转 10 NEX
const FUNDERS = {
  alice: getAlice,
  bob: getBob,
  charlie: getCharlie,
  dave: getDave,
  eve: getEve,
};

interface AccountInfo {
  index: number;
  mnemonic: string;
  address: string;
  publicKey: string;
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
  
  logSection(`创建 ${ACCOUNT_COUNT} 个测试账户`);
  
  const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  const accounts: AccountInfo[] = [];
  
  // ========================================
  // 步骤 1: 生成账户
  // ========================================
  logStep(1, `生成 ${ACCOUNT_COUNT} 个账户`);
  
  for (let i = 0; i < ACCOUNT_COUNT; i++) {
    const mnemonic = mnemonicGenerate();
    const pair = keyring.addFromMnemonic(mnemonic);
    
    accounts.push({
      index: i + 1,
      mnemonic,
      address: pair.address,
      publicKey: Buffer.from(pair.publicKey).toString('hex'),
    });
    
    console.log(`   账户 ${i + 1}: ${pair.address.slice(0, 16)}...`);
  }
  
  logSuccess(`已生成 ${ACCOUNT_COUNT} 个账户`);
  
  // ========================================
  // 步骤 2: 保存到文件
  // ========================================
  logStep(2, '保存账户信息到文件');
  
  const outputDir = process.cwd();
  const jsonFile = path.join(outputDir, 'test-accounts.json');
  const txtFile = path.join(outputDir, 'test-accounts.txt');
  
  // 保存 JSON 格式
  fs.writeFileSync(jsonFile, JSON.stringify(accounts, null, 2), 'utf-8');
  console.log(`   JSON 文件: ${jsonFile}`);
  
  // 保存可读文本格式
  let txtContent = `# 测试账户列表\n`;
  txtContent += `# 生成时间: ${new Date().toISOString()}\n`;
  txtContent += `# 账户数量: ${ACCOUNT_COUNT}\n`;
  txtContent += `${'='.repeat(80)}\n\n`;
  
  for (const acc of accounts) {
    txtContent += `账户 ${acc.index}:\n`;
    txtContent += `  地址: ${acc.address}\n`;
    txtContent += `  助记词: ${acc.mnemonic}\n`;
    txtContent += `  公钥: 0x${acc.publicKey}\n`;
    txtContent += `\n`;
  }
  
  fs.writeFileSync(txtFile, txtContent, 'utf-8');
  console.log(`   TXT 文件: ${txtFile}`);
  
  logSuccess('账户信息已保存');
  
  // ========================================
  // 步骤 3: 连接链并转账
  // ========================================
  logStep(3, '连接到链');
  
  const api = await getApi();
  const funder = FUNDERS[funderName]();
  
  logAccount(`${funderName} (发送方)`, funder);
  
  // 查询发送方余额
  const funderBalance = await api.query.system.account(funder.address);
  console.log(`   ${funderName} 余额: ${formatNex(funderBalance.data.free.toString())}`);
  
  const totalAmount = amount * ACCOUNT_COUNT;
  console.log(`   计划转账总额: ${totalAmount} NEX (每账户 ${amount} NEX)`);
  
  // ========================================
  // 步骤 4: 批量转账
  // ========================================
  logStep(4, `向 ${ACCOUNT_COUNT} 个账户转账`);
  
  const amountWei = toNexWei(amount);
  let successCount = 0;
  let failCount = 0;
  const utility = (api.tx as any).utility;

  if (utility?.batchAll) {
    const batchTx = utility.batchAll(
      accounts.map((acc) => api.tx.balances.transferKeepAlive(acc.address, amountWei))
    );
    const batchResult = await signAndSend(api, batchTx, funder, `批量转账给 ${ACCOUNT_COUNT} 个账户`);
    successCount = batchResult.success ? ACCOUNT_COUNT : 0;
    failCount = batchResult.success ? 0 : ACCOUNT_COUNT;

    if (!batchResult.success) {
      console.log(`   ❌ 批量转账失败: ${batchResult.error}`);
    }
  } else {
    console.log('   ℹ 当前链未启用 utility.batchAll，回退为逐笔转账');

    for (const acc of accounts) {
      console.log(`\n   转账给账户 ${acc.index}: ${acc.address.slice(0, 16)}...`);

      try {
        const transferTx = api.tx.balances.transferKeepAlive(acc.address, amountWei);
        const result = await signAndSend(api, transferTx, funder, `转账给账户 ${acc.index}`);

        if (result.success) {
          successCount++;
          console.log(`   ✅ 账户 ${acc.index} 转账成功`);
        } else {
          failCount++;
          console.log(`   ❌ 账户 ${acc.index} 转账失败: ${result.error}`);
        }
      } catch (error: any) {
        failCount++;
        console.log(`   ❌ 账户 ${acc.index} 转账异常: ${error.message}`);
      }
    }
  }
  
  // ========================================
  // 步骤 5: 验证转账结果
  // ========================================
  logStep(5, '验证转账结果');
  
  console.log(`\n   转账统计:`);
  console.log(`   - 成功: ${successCount}`);
  console.log(`   - 失败: ${failCount}`);
  
  // 随机抽查几个账户余额
  console.log(`\n   抽查账户余额:`);
  const checkIndices = [0, 9, 19]; // 第1、10、20个账户
  
  for (const idx of checkIndices) {
    if (idx < accounts.length) {
      const acc = accounts[idx];
      const balance = await api.query.system.account(acc.address);
      console.log(`   账户 ${acc.index}: ${formatNex(balance.data.free.toString())}`);
    }
  }
  
  // 查询发送方新余额
  const newFunderBalance = await api.query.system.account(funder.address);
  console.log(`\n   ${funderName} 新余额: ${formatNex(newFunderBalance.data.free.toString())}`);
  
  logSection('完成');
  
  if (failCount === 0) {
    logSuccess(`所有 ${ACCOUNT_COUNT} 个账户创建并转账成功！`);
  } else {
    logError(`${failCount} 个账户转账失败，${successCount} 个成功`);
  }
  
  console.log(`\n📁 账户文件:`);
  console.log(`   - ${jsonFile}`);
  console.log(`   - ${txtFile}`);
  
  await disconnectApi();
}

main().catch(console.error);
