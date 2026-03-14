#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';
process.env.POLKADOTJS_DISABLE_ESM_CJS_WARNING ??= '1';
const WS_URL = 'wss://202.140.140.202';

function parseSemverMajor(version) {
  const match = String(version ?? '').match(/^(\d+)/);
  return match ? Number(match[1]) : 0;
}
async function readJsonIfExists(file) {
  try { return JSON.parse(await fs.readFile(file, 'utf8')); } catch { return null; }
}
async function findPolkadotApiRoot() {
  const candidates = ['/home/xiaodong/node_modules', path.resolve(process.cwd(), 'node_modules')];
  const inspected = [];
  for (const root of [...new Set(candidates)]) {
    const pkg = await readJsonIfExists(path.join(root, '@polkadot/api/package.json'));
    if (pkg?.version) inspected.push({ root, version: pkg.version, major: parseSemverMajor(pkg.version) });
  }
  inspected.sort((a,b)=> b.major-a.major || b.version.localeCompare(a.version));
  const chosen = inspected.find(i=>i.major >=16) ?? inspected[0];
  if (!chosen) throw new Error('no api root');
  return { apiRoot: path.join(chosen.root, '@polkadot/api'), version: chosen.version };
}
const resolved = await findPolkadotApiRoot();
console.log('resolved', resolved);
const apiModule = await import(pathToFileURL(path.join(resolved.apiRoot, 'index.js')).href);
const { WsProvider, ApiPromise } = apiModule;
const provider = new WsProvider(WS_URL);
provider.on('connected', () => console.log('provider connected'));
provider.on('disconnected', () => console.log('provider disconnected'));
provider.on('error', (e) => console.log('provider error', e?.message ?? String(e)));
console.log('before create');
const timer = setInterval(() => console.log('tick', new Date().toISOString()), 5000);
try {
  const api = await ApiPromise.create({ provider });
  console.log('api ready', api.runtimeVersion.specName.toString(), api.runtimeVersion.specVersion.toString());
  await api.disconnect();
} finally {
  clearInterval(timer);
}
