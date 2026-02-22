/**
 * 测试账户工厂 — 为每条 Flow 提供隔离的角色账户
 */

import { Keyring } from '@polkadot/keyring';
import { KeyringPair } from '@polkadot/keyring/types';
import { ApiPromise } from '@polkadot/api';
import { signAndSend } from '../core/chain-state.js';
import { nex } from '../core/config.js';

const keyring = new Keyring({ type: 'sr25519' });

/** 开发链预置账户 */
export function getDevAccounts() {
  return {
    alice: keyring.addFromUri('//Alice'),
    bob: keyring.addFromUri('//Bob'),
    charlie: keyring.addFromUri('//Charlie'),
    dave: keyring.addFromUri('//Dave'),
    eve: keyring.addFromUri('//Eve'),
    ferdie: keyring.addFromUri('//Ferdie'),
  };
}

/**
 * 为指定 flow 创建隔离账户
 * URI 格式: //FlowPrefix/RoleName
 */
export function createFlowAccounts(
  flowPrefix: string,
  roles: string[],
): Record<string, KeyringPair> {
  const accounts: Record<string, KeyringPair> = {};
  // 始终包含 alice 作为 sudo
  accounts['alice'] = keyring.addFromUri('//Alice');

  for (const role of roles) {
    const uri = `//${flowPrefix}/${role}`;
    accounts[role.toLowerCase()] = keyring.addFromUri(uri);
  }
  return accounts;
}

/**
 * 给一组账户从 Alice 转账初始余额
 */
export async function fundAccounts(
  api: ApiPromise,
  accounts: Record<string, KeyringPair>,
  amountNex: number = 100_000,
): Promise<void> {
  const alice = keyring.addFromUri('//Alice');
  const amount = nex(amountNex).toString();

  for (const [name, account] of Object.entries(accounts)) {
    if (name === 'alice') continue;
    const tx = api.tx.balances.transferKeepAlive(account.address, amount);
    const result = await signAndSend(api, tx, alice, `Fund ${name}`);
    if (!result.success) {
      console.warn(`  ⚠ 向 ${name} 转账失败: ${result.error}`);
    }
  }
}
