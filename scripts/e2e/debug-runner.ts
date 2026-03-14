#!/usr/bin/env tsx

const DEFAULT_REMOTE_WS_URL = 'wss://202.140.140.202';
process.env.WS_URL ??= DEFAULT_REMOTE_WS_URL;
process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';

const { connectApi, disconnectApi, captureChainSnapshot } = await import('./framework/api.js');
const { getDevActors } = await import('./framework/accounts.js');

async function main() {
  console.log('debug: before connect');
  const api = await connectApi(process.env.WS_URL);
  try {
    console.log('debug: after connect');
    const chain = await captureChainSnapshot(api);
    console.log('debug: chain', JSON.stringify(chain));
    const actors = await getDevActors();
    console.log('debug: actors', Object.keys(actors).join(','));
  } finally {
    await disconnectApi(api);
    console.log('debug: disconnected');
  }
}

main().catch((error) => {
  console.error('debug runner failed:', error);
  process.exitCode = 1;
});
