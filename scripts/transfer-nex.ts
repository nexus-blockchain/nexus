#!/usr/bin/env tsx

process.env.WS_URL ??= 'wss://202.140.140.202';
process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';

import { connectApi, disconnectApi, submitTx } from './e2e/framework/api.js';
import { getDevActors, readFreeBalance } from './e2e/framework/accounts.js';
import { assert, assertTxSuccess } from './e2e/framework/assert.js';
import { NEX_PLANCK, formatNex } from './e2e/framework/units.js';

function parseNex(value: string): bigint {
  const trimmed = value.trim();
  assert(/^\d+(\.\d+)?$/.test(trimmed), `invalid NEX amount: ${value}`);
  const [whole, fraction = ''] = trimmed.split('.');
  const fractionPadded = `${fraction}000000000000`.slice(0, 12);
  return BigInt(whole) * NEX_PLANCK + BigInt(fractionPadded);
}

function formatPlanck(raw: bigint): string {
  const sign = raw < 0n ? '-' : '';
  const abs = raw < 0n ? -raw : raw;
  const whole = abs / NEX_PLANCK;
  const frac = (abs % NEX_PLANCK).toString().padStart(12, '0').replace(/0+$/, '');
  return frac ? `${sign}${whole}.${frac}` : `${sign}${whole}`;
}

async function main(): Promise<void> {
  const [, , targetAddress, amountArg] = process.argv;
  assert(targetAddress, 'usage: node --import tsx transfer-nex.ts <target-address> <amount-nex>');
  assert(amountArg, 'usage: node --import tsx transfer-nex.ts <target-address> <amount-nex>');

  const requested = parseNex(amountArg);
  assert(requested > 0n, 'amount must be greater than 0');

  const senderReserve = parseNex(process.env.TRANSFER_SENDER_RESERVE_NEX ?? '100');
  const senderNames = (process.env.TRANSFER_ACTORS ?? 'alice,bob,charlie,dave,eve,ferdie')
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);

  const api = await connectApi(process.env.WS_URL);

  try {
    const actors = await getDevActors();
    const targetBefore = await readFreeBalance(api, targetAddress);

    const balances = await Promise.all(senderNames.map(async (name) => {
      const actor = actors[name];
      assert(actor, `unknown actor: ${name}`);
      const free = await readFreeBalance(api, actor.address);
      const spendable = free > senderReserve ? free - senderReserve : 0n;
      return { name, actor, free, spendable };
    }));

    balances.sort((left, right) => (left.spendable === right.spendable ? 0 : left.spendable > right.spendable ? -1 : 1));

    const totalSpendable = balances.reduce((sum, item) => sum + item.spendable, 0n);
    assert(
      totalSpendable >= requested,
      `insufficient spendable dev balance: need ${formatPlanck(requested)} NEX, have ${formatPlanck(totalSpendable)} NEX`,
    );

    let remaining = requested;
    const plan = balances
      .map((item) => {
        if (remaining <= 0n || item.spendable <= 0n) {
          return { ...item, send: 0n };
        }
        const send = item.spendable >= remaining ? remaining : item.spendable;
        remaining -= send;
        return { ...item, send };
      })
      .filter((item) => item.send > 0n);

    assert(remaining === 0n, `failed to allocate full transfer amount, remaining=${formatPlanck(remaining)} NEX`);

    const receipts: Array<{
      sender: string;
      senderAddress: string;
      amountPlanck: string;
      amountNex: string;
      txHash: string;
      blockHash?: string;
    }> = [];

    for (const step of plan) {
      const tx = (api.tx.balances as any).transferKeepAlive(targetAddress, step.send.toString());
      const receipt = await submitTx(api, tx, step.actor, `transfer ${formatPlanck(step.send)} NEX from ${step.name}`);
      assertTxSuccess(receipt, `transfer from ${step.name} should succeed`);
      receipts.push({
        sender: step.name,
        senderAddress: step.actor.address,
        amountPlanck: step.send.toString(),
        amountNex: formatPlanck(step.send),
        txHash: receipt.txHash,
        blockHash: receipt.blockHash,
      });
    }

    const targetAfter = await readFreeBalance(api, targetAddress);

    console.log(JSON.stringify({
      network: process.env.WS_URL,
      targetAddress,
      requested: {
        planck: requested.toString(),
        nex: formatPlanck(requested),
      },
      targetBalance: {
        beforePlanck: targetBefore.toString(),
        beforeFormatted: formatNex(targetBefore),
        afterPlanck: targetAfter.toString(),
        afterFormatted: formatNex(targetAfter),
        deltaPlanck: (targetAfter - targetBefore).toString(),
        deltaNex: formatPlanck(targetAfter - targetBefore),
      },
      reservePerSender: {
        planck: senderReserve.toString(),
        nex: formatPlanck(senderReserve),
      },
      receipts,
    }, null, 2));
  } finally {
    await disconnectApi(api);
  }
}

main().catch((error) => {
  console.error(`transfer NEX failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
