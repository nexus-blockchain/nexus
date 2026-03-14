#!/usr/bin/env tsx

process.env.WS_URL ??= 'wss://202.140.140.202';
process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';

const { connectApi, disconnectApi, submitTx } = await import('./framework/api.js');
const { getDevActors, readFreeBalance } = await import('./framework/accounts.js');

async function main(): Promise<void> {
  console.log('debug: before connect');
  const api = await connectApi(process.env.WS_URL);
  try {
    console.log('debug: connected');
    console.log('debug: loading actors');
    const actors = await getDevActors();
    console.log('debug: actors ready');
    const before = await readFreeBalance(api, actors.dave.address);
    console.log('daveBefore', before.toString());
    const tx = api.tx.balances.transferKeepAlive(actors.dave.address, '1000000000000');
    const receipt = await submitTx(api, tx, actors.alice, 'fund dave');
    console.log(JSON.stringify(receipt, null, 2));
    const after = await readFreeBalance(api, actors.dave.address);
    console.log('daveAfter', after.toString());
  } finally {
    await disconnectApi(api);
  }
}

main().catch((error) => {
  console.error(`debug transfer failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
