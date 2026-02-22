/**
 * 转账工具脚本
 * 从 Alice 转账给指定账户
 * 
 * 使用方法:
 *   npx tsx transfer.ts <recipient> <amount>
 *   
 * 示例:
 *   npx tsx transfer.ts charlie 1000    # 转 1000 NEX 给 Charlie
 *   npx tsx transfer.ts dave 500        # 转 500 NEX 给 Dave
 */

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

const ACCOUNTS: Record<string, () => any> = {
  alice: getAlice,
  bob: getBob,
  charlie: getCharlie,
  dave: getDave,
  eve: getEve,
};

async function main() {
  const args = process.argv.slice(2);
  
  if (args.length < 2) {
    console.log('使用方法: npx tsx transfer.ts <recipient> <amount>');
    console.log('示例: npx tsx transfer.ts charlie 1000');
    console.log('\n可用账户: alice, bob, charlie, dave, eve');
    return;
  }
  
  const recipientName = args[0].toLowerCase();
  const amount = parseFloat(args[1]);
  
  if (!ACCOUNTS[recipientName]) {
    logError(`未知账户: ${recipientName}`);
    console.log('可用账户: alice, bob, charlie, dave, eve');
    return;
  }
  
  if (isNaN(amount) || amount <= 0) {
    logError('金额必须是正数');
    return;
  }
  
  logSection('转账工具');
  
  const api = await getApi();
  const alice = getAlice();
  const recipient = ACCOUNTS[recipientName]();
  
  logAccount('Alice (发送方)', alice);
  logAccount(`${recipientName} (接收方)`, recipient);
  
  try {
    // 查询余额
    logStep(1, '查询余额');
    
    const aliceBalance = await api.query.system.account(alice.address);
    const recipientBalance = await api.query.system.account(recipient.address);
    
    console.log(`   Alice 余额: ${formatNex(aliceBalance.data.free.toString())}`);
    console.log(`   ${recipientName} 余额: ${formatNex(recipientBalance.data.free.toString())}`);
    
    // 转账
    logStep(2, `转账 ${amount} NEX`);
    
    const amountWei = toNexWei(amount);
    console.log(`   转账金额: ${formatNex(amountWei)}`);
    
    const transferTx = api.tx.balances.transferKeepAlive(recipient.address, amountWei);
    const result = await signAndSend(api, transferTx, alice, `Alice 转账给 ${recipientName}`);
    
    if (!result.success) {
      logError(`转账失败: ${result.error}`);
      return;
    }
    
    logSuccess('转账成功！');
    
    // 查询新余额
    logStep(3, '查询新余额');
    
    const newAliceBalance = await api.query.system.account(alice.address);
    const newRecipientBalance = await api.query.system.account(recipient.address);
    
    console.log(`   Alice 余额: ${formatNex(newAliceBalance.data.free.toString())}`);
    console.log(`   ${recipientName} 余额: ${formatNex(newRecipientBalance.data.free.toString())}`);
    
    logSection('完成');
    
  } catch (error: any) {
    logError(`执行失败: ${error.message}`);
    console.error(error);
  } finally {
    await disconnectApi();
  }
}

main().catch(console.error);
