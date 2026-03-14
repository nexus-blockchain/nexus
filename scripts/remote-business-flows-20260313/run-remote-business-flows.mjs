#!/usr/bin/env node

import { promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';
process.env.POLKADOTJS_DISABLE_ESM_CJS_WARNING ??= '1';

const scriptFile =
  process.env.REMOTE_FLOW_SCRIPT_FILE
  ?? (String(import.meta.url).startsWith('file:') ? fileURLToPath(import.meta.url) : path.join(process.cwd(), 'remote-business-flows-20260313', 'run-remote-business-flows.mjs'));

const __filename = scriptFile;
const __dirname = path.dirname(__filename);

const WS_URL = process.env.WS_URL ?? 'wss://202.140.140.202';
const REPORT_DIR = __dirname;
const ARTIFACT_DIR = path.join(REPORT_DIR, 'artifacts');
const PROGRESS_DIR = path.join(ARTIFACT_DIR, 'progress');
const CASE_PROGRESS_DIR = path.join(PROGRESS_DIR, 'cases');
const BOOTSTRAP_STATUS_PATH = path.join(ARTIFACT_DIR, 'bootstrap-status.json');
const BOOTSTRAP_LOG_PATH = path.join(ARTIFACT_DIR, 'bootstrap.log');
const LIVE_LOG_PATH = path.join(ARTIFACT_DIR, 'live.log');
const LATEST_JSON_PATH = path.join(ARTIFACT_DIR, 'latest.json');
const STATUS_JSON_PATH = path.join(ARTIFACT_DIR, 'status.json');
const EXECUTION_STATUS_PATH = path.join(ARTIFACT_DIR, 'execution-status.json');
const REPORT_PATH = path.join(REPORT_DIR, 'REPORT.md');
const NEX_PLANCK = 1_000_000_000_000n;
const SS58_FORMAT = 273;
const REPORT_DATE = '2026-03-13';
const POLKADOT_API_VERSION = process.env.POLKADOT_API_VERSION ?? '16.5.4';
const POLKADOT_API_ROOT = process.env.POLKADOT_API_ROOT ?? path.resolve(process.cwd(), 'node_modules/@polkadot/api');

const CASE_CATALOG = [
  {
    id: 'entity-shop-flow',
    title: 'Entity shop extended lifecycle',
    modules: ['pallet-entity-shop'],
  },
  {
    id: 'entity-member-loyalty-flow',
    title: 'Approval onboarding + points issue/transfer/redeem',
    modules: ['pallet-entity-member', 'pallet-entity-loyalty'],
  },
  {
    id: 'entity-product-order-physical-flow',
    title: 'Physical product lifecycle + shipping + refund',
    modules: ['pallet-entity-product', 'pallet-entity-order'],
  },
  {
    id: 'commission-admin-controls',
    title: 'Commission plugin control-plane flows',
    modules: [
      'pallet-commission-single-line',
      'pallet-commission-multi-level',
      'pallet-commission-pool-reward',
    ],
  },
  {
    id: 'nex-market-trade-flow',
    title: 'Matched sell order → reserve → payment confirmation → seller settlement',
    modules: ['pallet-nex-market'],
  },
];

function parseCliArgs(argv) {
  const requestedCases = [];
  let list = false;
  let help = false;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--list') {
      list = true;
      continue;
    }

    if (arg === '--help' || arg === '-h') {
      help = true;
      continue;
    }

    if (arg === '--case') {
      const next = argv[index + 1];
      if (!next) {
        throw new Error('Missing value after --case');
      }
      requestedCases.push(...next.split(',').map((item) => item.trim()).filter(Boolean));
      index += 1;
      continue;
    }

    if (arg.startsWith('--case=')) {
      requestedCases.push(...arg.slice('--case='.length).split(',').map((item) => item.trim()).filter(Boolean));
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  const envCases = String(process.env.REMOTE_FLOW_CASES ?? '')
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);

  const selectedCases = [...new Set([...requestedCases, ...envCases])];
  const knownCaseIds = new Set(CASE_CATALOG.map((item) => item.id));
  const unknownCaseIds = selectedCases.filter((item) => !knownCaseIds.has(item));
  if (unknownCaseIds.length > 0) {
    throw new Error(`Unknown case ids: ${unknownCaseIds.join(', ')}`);
  }

  return {
    list,
    help,
    selectedCases,
  };
}

const CLI = parseCliArgs(process.argv.slice(2));

function printHelp() {
  console.log(`Usage: node remote-business-flows-20260313/run-remote-business-flows.mjs [--list] [--case <id[,id...]>]`);
  console.log('');
  console.log('Options:');
  console.log('  --list              List runnable remote business-flow cases');
  console.log('  --case <ids>        Run only the specified case ids (comma-separated or repeatable)');
  console.log('  --help              Show this help');
  console.log('');
  console.log('Environment variables:');
  console.log('  WS_URL                        Override remote websocket URL');
  console.log('  REMOTE_FLOW_CASES            Case filter, comma-separated');
  console.log('  REMOTE_FLOW_TX_TIMEOUT_MS    Per-tx finalize timeout');
  console.log('  REMOTE_FLOW_HEARTBEAT_MS     Artifact heartbeat persist interval');
}

if (CLI.help) {
  printHelp();
  process.exit(0);
}

if (CLI.list) {
  for (const entry of CASE_CATALOG) {
    console.log(`${entry.id}\t${entry.title}\t${entry.modules.join(', ')}`);
  }
  process.exit(0);
}

const VALID_TRON_ADDRESSES = {
  seller: 'TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t',
  buyer: 'TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE',
};

function nex(amount) {
  return BigInt(Math.round(amount * 1_000_000_000_000));
}

function formatNex(raw) {
  const value = typeof raw === 'bigint' ? raw : BigInt(String(raw));
  return `${(Number(value) / 1e12).toLocaleString()} NEX`;
}

function uniqueSuffix() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function toJson(value) {
  if (value == null) {
    return value;
  }
  if (typeof value.toJSON === 'function') {
    return value.toJSON();
  }
  return value;
}

function toStringMaybe(value) {
  if (value == null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'bigint' || typeof value === 'boolean') {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.map((item) => toStringMaybe(item)).join(',');
  }
  if (typeof value?.toString === 'function') {
    return value.toString();
  }
  return JSON.stringify(value);
}

function jsonSafe(value) {
  return JSON.parse(JSON.stringify(value, (_, current) => {
    if (typeof current === 'bigint') {
      return current.toString();
    }
    return current;
  }));
}

function readField(record, ...keys) {
  if (!record || typeof record !== 'object') {
    return undefined;
  }

  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(record, key)) {
      return record[key];
    }
  }

  return undefined;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function normalizeEvent(record) {
  return {
    section: record.event.section,
    method: record.event.method,
    name: `${record.event.section}.${record.event.method}`,
    data: toJson(record.event.data),
  };
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function writeTextAtomic(file, content) {
  await ensureDir(path.dirname(file));
  const temp = `${file}.${process.pid}.${Date.now()}.tmp`;
  await fs.writeFile(temp, content, 'utf8');
  await fs.rename(temp, file);
}

async function writeJsonAtomic(file, value) {
  await writeTextAtomic(file, `${JSON.stringify(jsonSafe(value), null, 2)}\n`);
}

async function appendLine(file, line) {
  await ensureDir(path.dirname(file));
  await fs.appendFile(file, `${line}\n`, 'utf8');
}

function makeSkippedFlows() {
  return [
    {
      module: 'pallet-entity-shop',
      flow: 'Entity 自动创建主店 + 基础资金充值',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-entity-member',
      flow: '开放注册 + 推荐链注册 + 激活会员',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-entity-loyalty',
      flow: '佣金提现到 shopping balance + 使用 shopping balance 下单',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-entity-product',
      flow: '数字商品创建/发布 + MembersOnly 可见性',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-entity-order',
      flow: '数字商品即时完成订单',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-commission-single-line',
      flow: '单线索引分配 + 订单触发分润',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-commission-multi-level',
      flow: '多级分佣随订单发放',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-commission-pool-reward',
      flow: '沉淀池累积 + claimPoolReward',
      existingSuite: 'e2e/suites/entity-commerce-commission-flow.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'pallet-nex-market',
      flow: '单纯挂买/挂卖并撤单',
      existingSuite: 'e2e/suites/nex-market-smoke.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
    {
      module: 'entity registry',
      flow: 'createEntity + updateEntity 元数据冒烟',
      existingSuite: 'e2e/suites/entity-lifecycle.ts',
      reason: '已有远程写流覆盖，按要求跳过重复验证。',
    },
  ];
}

async function writeBootstrapStatus(stage, extra = {}) {
  const payload = {
    updatedAt: new Date().toISOString(),
    pid: process.pid,
    wsUrl: WS_URL,
    stage,
    ...extra,
  };
  await writeJsonAtomic(BOOTSTRAP_STATUS_PATH, payload);
}

async function bootstrapLog(message) {
  const line = `[${new Date().toISOString()}] ${message}`;
  console.log(message);
  await appendLine(BOOTSTRAP_LOG_PATH, line);
}

class Reporter {
  constructor(meta, paths) {
    this.meta = meta;
    this.paths = paths;
    this.persistChain = Promise.resolve();
    this.heartbeatTimer = null;
    this.results = {
      meta,
      skippedAlreadyCovered: makeSkippedFlows(),
      cases: [],
      summary: {
        passed: 0,
        failed: 0,
        skipped: 0,
      },
    };
  }

  queuePersist(reason, context = {}) {
    this.results.meta.lastUpdatedAt = new Date().toISOString();
    this.results.meta.lastPersistReason = reason;
    this.results.meta.currentActivity = {
      reason,
      caseId: context.caseEntry?.id ?? null,
      stepTitle: context.stepEntry?.title ?? null,
      updatedAt: this.results.meta.lastUpdatedAt,
    };

    this.persistChain = this.persistChain
      .then(async () => {
        const snapshot = jsonSafe(this.results);

        await Promise.all([
          writeJsonAtomic(this.paths.latestJsonPath, snapshot),
          writeJsonAtomic(this.paths.statusJsonPath, {
            updatedAt: snapshot.meta.lastUpdatedAt,
            pid: process.pid,
            wsUrl: snapshot.meta.wsUrl,
            reason,
            summary: snapshot.summary,
            currentActivity: snapshot.meta.currentActivity,
          }),
          writeTextAtomic(this.paths.reportPath, buildMarkdownReport(snapshot)),
          context.caseEntry
            ? writeJsonAtomic(path.join(this.paths.caseProgressDir, `${context.caseEntry.id}.json`), context.caseEntry)
            : Promise.resolve(),
        ]);
      })
      .catch(async (error) => {
        const line = `[${new Date().toISOString()}] persist error (${reason}): ${error instanceof Error ? error.stack ?? error.message : String(error)}`;
        console.error(line);
        await appendLine(this.paths.liveLogPath, line);
      });

    return this.persistChain;
  }

  async log(message) {
    console.log(message);
    await appendLine(this.paths.liveLogPath, `[${new Date().toISOString()}] ${message}`);
  }

  startHeartbeat(intervalMs = Number(process.env.REMOTE_FLOW_HEARTBEAT_MS ?? 10_000)) {
    if (this.heartbeatTimer || intervalMs <= 0) {
      return;
    }

    this.heartbeatTimer = setInterval(() => {
      void this.queuePersist('heartbeat');
    }, intervalMs);
  }

  stopHeartbeat() {
    if (!this.heartbeatTimer) {
      return;
    }
    clearInterval(this.heartbeatTimer);
    this.heartbeatTimer = null;
  }

  async createCase(id, title, modules) {
    const entry = {
      id,
      title,
      modules,
      status: 'running',
      startedAt: new Date().toISOString(),
      endedAt: null,
      notes: [],
      steps: [],
      error: null,
    };
    this.results.cases.push(entry);
    await this.queuePersist('case-created', { caseEntry: entry });
    return entry;
  }

  async note(caseEntry, message) {
    caseEntry.notes.push(message);
    await this.log(`    • ${message}`);
    await this.queuePersist('case-note', { caseEntry });
  }

  async step(caseEntry, title, action) {
    const startedAt = Date.now();
    const step = {
      title,
      status: 'running',
      startedAt: new Date().toISOString(),
      endedAt: null,
      durationMs: null,
      output: null,
      error: null,
    };
    caseEntry.steps.push(step);
    await this.log(`  → ${title}`);
    await this.queuePersist('step-start', { caseEntry, stepEntry: step });

    try {
      const output = await action();
      step.status = 'passed';
      step.endedAt = new Date().toISOString();
      step.durationMs = Date.now() - startedAt;
      step.output = output == null ? null : jsonSafe(output);
      await this.log(`    ✓ ${title} (${step.durationMs}ms)`);
      await this.queuePersist('step-passed', { caseEntry, stepEntry: step });
      return output;
    } catch (error) {
      step.status = 'failed';
      step.endedAt = new Date().toISOString();
      step.durationMs = Date.now() - startedAt;
      step.error = error instanceof Error ? error.message : String(error);
      await this.log(`    ✗ ${title} (${step.durationMs}ms) — ${step.error}`);
      await this.queuePersist('step-failed', { caseEntry, stepEntry: step });
      throw error;
    }
  }

  async pass(caseEntry) {
    caseEntry.status = 'passed';
    caseEntry.endedAt = new Date().toISOString();
    this.results.summary.passed += 1;
    await this.queuePersist('case-passed', { caseEntry });
  }

  async fail(caseEntry, error) {
    caseEntry.status = 'failed';
    caseEntry.endedAt = new Date().toISOString();
    caseEntry.error = error instanceof Error ? error.message : String(error);
    this.results.summary.failed += 1;
    await this.queuePersist('case-failed', { caseEntry });
  }

  async skip(caseEntry, reason) {
    caseEntry.status = 'skipped';
    caseEntry.endedAt = new Date().toISOString();
    caseEntry.error = reason;
    this.results.summary.skipped += 1;
    await this.queuePersist('case-skipped', { caseEntry });
  }

  async finalize() {
    this.stopHeartbeat();
    await this.queuePersist('finalize');
  }
}

async function main() {
  await ensureDir(ARTIFACT_DIR);
  await ensureDir(PROGRESS_DIR);
  await ensureDir(CASE_PROGRESS_DIR);

  await writeBootstrapStatus('locating-polkadot-api');
  await bootstrapLog('[bootstrap] resolving polkadot api packages');
  await writeBootstrapStatus('selected-polkadot-api', { apiVersion: POLKADOT_API_VERSION, apiRoot: POLKADOT_API_ROOT });
  await bootstrapLog(`[bootstrap] selected @polkadot/api ${POLKADOT_API_VERSION} from ${POLKADOT_API_ROOT}`);
  await writeBootstrapStatus('waiting-crypto');
  await bootstrapLog('[bootstrap] waiting for crypto');
  await cryptoWaitReady();
  await writeBootstrapStatus('connecting-api');
  await bootstrapLog('[bootstrap] connecting api');

  const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
  await writeBootstrapStatus('api-connected');
  await bootstrapLog('[bootstrap] api connected');

  const keyring = new Keyring({ type: 'sr25519', ss58Format: SS58_FORMAT });
  const actors = {
    alice: keyring.addFromUri('//Alice'),
    bob: keyring.addFromUri('//Bob'),
    charlie: keyring.addFromUri('//Charlie'),
    dave: keyring.addFromUri('//Dave'),
    eve: keyring.addFromUri('//Eve'),
    ferdie: keyring.addFromUri('//Ferdie'),
  };

  const [chain, nodeName, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version(),
  ]);

  const reporter = new Reporter({
    reportDate: REPORT_DATE,
    startedAt: new Date().toISOString(),
    wsUrl: WS_URL,
    selectedCases: CLI.selectedCases,
    chain: chain.toString(),
    nodeName: nodeName.toString(),
    nodeVersion: nodeVersion.toString(),
    specName: api.runtimeVersion.specName.toString(),
    specVersion: api.runtimeVersion.specVersion.toString(),
    apiVersion: POLKADOT_API_VERSION,
    apiRoot: POLKADOT_API_ROOT,
  }, {
    latestJsonPath: LATEST_JSON_PATH,
    statusJsonPath: STATUS_JSON_PATH,
    reportPath: REPORT_PATH,
    liveLogPath: LIVE_LOG_PATH,
    caseProgressDir: CASE_PROGRESS_DIR,
  });
  reporter.startHeartbeat();
  await reporter.queuePersist('reporter-created');

  await reporter.log(`Connected to ${reporter.meta.chain} / ${reporter.meta.nodeName} ${reporter.meta.nodeVersion}`);
  await reporter.log(`Runtime ${reporter.meta.specName} v${reporter.meta.specVersion}`);
  await reporter.log(`Using @polkadot/api ${reporter.meta.apiVersion} from ${reporter.meta.apiRoot}`);

  const txTimeoutMs = Number(process.env.REMOTE_FLOW_TX_TIMEOUT_MS ?? 180_000);

  function decodeDispatchError(dispatchError) {
    if (!dispatchError) {
      return null;
    }
    if (dispatchError.isModule) {
      const meta = api.registry.findMetaError(dispatchError.asModule);
      return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
    }
    return dispatchError.toString();
  }

  async function submitTx(tx, signer, label) {
    return await new Promise((resolve, reject) => {
      let unsubscribe;
      let latest;
      const timeout = setTimeout(() => {
        reject(new Error(`Timed out waiting for finalized status: ${label}`));
      }, txTimeoutMs);

      tx.signAndSend(signer, async (result) => {
        if (result.status.isInBlock || result.status.isFinalized) {
          latest = {
            txHash: tx.hash.toHex(),
            status: result.status.type,
            dispatchError: result.dispatchError,
            events: result.events.map(normalizeEvent),
          };
        }

        if (!result.status.isFinalized) {
          return;
        }

        clearTimeout(timeout);

        try {
          if (unsubscribe) {
            await unsubscribe();
          }
        } catch {
          // ignore unsubscribe errors
        }

        const receipt = latest ?? {
          txHash: tx.hash.toHex(),
          status: result.status.type,
          dispatchError: result.dispatchError,
          events: result.events.map(normalizeEvent),
        };

        const errorMessage = decodeDispatchError(receipt.dispatchError);
        if (errorMessage) {
          reject(new Error(`${label} failed: ${errorMessage}`));
          return;
        }

        resolve({
          txHash: receipt.txHash,
          status: receipt.status,
          events: receipt.events,
        });
      }).then((unsub) => {
        unsubscribe = unsub;
      }).catch((error) => {
        clearTimeout(timeout);
        reject(error);
      });
    });
  }

  async function readFreeBalance(address) {
    const account = await api.query.system.account(address);
    return BigInt(account.data.free.toString());
  }

  async function ensureNamedBalance(name, minNex) {
    const actor = actors[name];
    assert(actor, `Unknown actor ${name}`);
    if (name === 'alice') {
      return readFreeBalance(actor.address);
    }

    const minimum = nex(minNex);
    const current = await readFreeBalance(actor.address);
    if (current >= minimum) {
      return current;
    }

    const delta = minimum - current;
    await submitTx(api.tx.balances.transferKeepAlive(actor.address, delta.toString()), actors.alice, `fund ${name}`);
    return readFreeBalance(actor.address);
  }

  async function chooseOwner() {
    for (const name of ['alice', 'dave', 'ferdie', 'charlie']) {
      const entityIds = toJson(await api.query.entityRegistry.userEntity(actors[name].address));
      if (Array.isArray(entityIds) && entityIds.length < 3) {
        return { name, actor: actors[name], entityIds };
      }
    }
    throw new Error('No dev actor has remaining entity capacity');
  }

  async function readEntity(entityId) {
    const value = await api.query.entityRegistry.entities(entityId);
    assert(value.isSome, `Entity ${entityId} should exist`);
    return toJson(value.unwrap());
  }

  async function readShop(shopId) {
    const value = await api.query.entityShop.shops(shopId);
    assert(value.isSome, `Shop ${shopId} should exist`);
    return toJson(value.unwrap());
  }

  async function readProduct(productId) {
    const value = await api.query.entityProduct.products(productId);
    assert(value.isSome, `Product ${productId} should exist`);
    return toJson(value.unwrap());
  }

  async function readOrder(orderId) {
    const value = await api.query.entityTransaction.orders(orderId);
    assert(value.isSome, `Order ${orderId} should exist`);
    return toJson(value.unwrap());
  }

  async function readPendingMember(entityId, address) {
    const value = await api.query.entityMember.pendingMembers(entityId, address);
    return toJson(value);
  }

  const state = {
    baseContext: null,
    baseContextError: null,
  };

  async function ensureBaseContext() {
    if (state.baseContext) {
      return state.baseContext;
    }
    if (state.baseContextError) {
      throw state.baseContextError;
    }

    try {
      const ownerInfo = await chooseOwner();
      const owner = ownerInfo.actor;
      const baseName = `rbf-${uniqueSuffix()}`;
      const secondaryShopName = `rbf-shop-${uniqueSuffix()}`;
      const nextEntityId = Number((await api.query.entityRegistry.nextEntityId()).toString());

      await submitTx(
        api.tx.entityRegistry.createEntity(baseName, null, null, null),
        owner,
        'create base entity',
      );

      const entityId = nextEntityId;
      const entity = await readEntity(entityId);
      const primaryShopId = Number(readField(entity, 'primaryShopId', 'primary_shop_id'));
      assert(primaryShopId > 0, 'Expected auto-created primary shop');

      const nextShopId = Number((await api.query.entityShop.nextShopId()).toString());

      await submitTx(
        api.tx.entityShop.createShop(entityId, secondaryShopName, 'OnlineStore', 0),
        owner,
        'create secondary shop',
      );

      const secondaryShopId = nextShopId;
      await submitTx(
        api.tx.entityShop.addManager(secondaryShopId, actors.bob.address),
        owner,
        'add bob as shop manager',
      );

      await submitTx(
        api.tx.entityShop.updateShop(
          secondaryShopId,
          `rbf-shop-updated-${uniqueSuffix()}`,
          null,
          null,
          null,
          null,
        ),
        actors.bob,
        'manager updates shop metadata',
      );

      await submitTx(
        api.tx.entityShop.setPrimaryShop(entityId, secondaryShopId),
        owner,
        'set secondary shop as primary',
      );

      const initialOperatingFund = nex(5_000);
      await submitTx(
        api.tx.entityShop.fundOperating(secondaryShopId, initialOperatingFund.toString()),
        owner,
        'fund operating balance',
      );

      const withdrawAmount = nex(50);
      await submitTx(
        api.tx.entityShop.withdrawOperatingFund(secondaryShopId, withdrawAmount.toString()),
        owner,
        'withdraw a small operating balance',
      );

      await submitTx(
        api.tx.entityShop.removeManager(secondaryShopId, actors.bob.address),
        owner,
        'remove bob as shop manager',
      );

      const shop = await readShop(secondaryShopId);
      const entityAfter = await readEntity(entityId);

      state.baseContext = {
        ownerName: ownerInfo.name,
        owner,
        entityId,
        primaryShopId,
        shopId: secondaryShopId,
        initialOperatingFund: initialOperatingFund.toString(),
        withdrawnOperatingFund: withdrawAmount.toString(),
        entityAfter,
        shopAfter: shop,
      };

      return state.baseContext;
    } catch (error) {
      state.baseContextError = error instanceof Error ? error : new Error(String(error));
      throw state.baseContextError;
    }
  }

  async function runCase(id, title, modules, action) {
    if (CLI.selectedCases.length > 0 && !CLI.selectedCases.includes(id)) {
      return;
    }

    const caseEntry = await reporter.createCase(id, title, modules);
    await reporter.log(`\n• ${title} [${id}]`);
    try {
      await action(caseEntry);
      await reporter.pass(caseEntry);
      await reporter.log(`  ✓ case passed`);
    } catch (error) {
      await reporter.fail(caseEntry, error);
      await reporter.log(`  ! case failed — ${caseEntry.error}`);
    }
  }

  await runCase(
    'entity-shop-flow',
    'Entity shop extended lifecycle',
    ['pallet-entity-shop'],
    async (caseEntry) => {
      const base = await reporter.step(caseEntry, 'create base entity and secondary shop context', async () => {
        return ensureBaseContext();
      });

      await reporter.step(caseEntry, 'verify secondary shop became the entity primary shop', async () => {
        const entity = await readEntity(base.entityId);
        const primaryShopId = Number(readField(entity, 'primaryShopId', 'primary_shop_id'));
        assert(primaryShopId === base.shopId, `Expected primaryShopId=${base.shopId}, got ${primaryShopId}`);
        return {
          entityId: base.entityId,
          primaryShopId,
        };
      });

      await reporter.step(caseEntry, 'verify manager removal and operating balance mutations are reflected in storage', async () => {
        const shop = await readShop(base.shopId);
        const managers = readField(shop, 'managers') ?? [];
        assert(Array.isArray(managers), 'shop.managers should be an array');
        assert(!managers.includes(actors.bob.address), 'bob should have been removed from managers');
        return {
          shopId: base.shopId,
          managers,
          shop,
        };
      });
    },
  );

  await runCase(
    'entity-member-loyalty-flow',
    'Approval onboarding + points issue/transfer/redeem',
    ['pallet-entity-member', 'pallet-entity-loyalty'],
    async (caseEntry) => {
      const base = await ensureBaseContext();

      await reporter.step(caseEntry, 'ensure participating actors have sufficient balances', async () => {
        const balances = {
          charlie: (await ensureNamedBalance('charlie', 5_000)).toString(),
          dave: (await ensureNamedBalance('dave', 5_000)).toString(),
        };
        return balances;
      });

      await reporter.step(caseEntry, 'switch member policy to approval-required and capture pending registrations', async () => {
        await submitTx(
          api.tx.entityMember.setMemberPolicy(base.shopId, 4),
          base.owner,
          'set approval-required member policy',
        );

        await submitTx(
          api.tx.entityMember.registerMember(base.shopId, null),
          actors.charlie,
          'charlie register member pending',
        );

        await submitTx(
          api.tx.entityMember.registerMember(base.shopId, null),
          actors.dave,
          'dave register member pending',
        );

        const pendingCharlie = await readPendingMember(base.entityId, actors.charlie.address);
        const pendingDave = await readPendingMember(base.entityId, actors.dave.address);
        assert(pendingCharlie != null, 'charlie should have a pending member record');
        assert(pendingDave != null, 'dave should have a pending member record');

        return {
          pendingCharlie,
          pendingDave,
        };
      });

      await reporter.step(caseEntry, 'batch approve the pending members', async () => {
        await submitTx(
          api.tx.entityMember.batchApproveMembers(base.shopId, [actors.charlie.address, actors.dave.address]),
          base.owner,
          'batch approve members',
        );

        const memberCharlie = await api.query.entityMember.entityMembers(base.entityId, actors.charlie.address);
        const memberDave = await api.query.entityMember.entityMembers(base.entityId, actors.dave.address);
        assert(memberCharlie.isSome, 'charlie should now be an approved member');
        assert(memberDave.isSome, 'dave should now be an approved member');

        return {
          charlieMember: toJson(memberCharlie.unwrap()),
          daveMember: toJson(memberDave.unwrap()),
        };
      });

      await reporter.step(caseEntry, 'enable points, issue to Charlie, transfer to Dave, then redeem part of Dave balance', async () => {
        await submitTx(
          api.tx.entityLoyalty.enablePoints(base.shopId, 'RemoteFlowPts', 'RFP', 500, 10_000, true),
          base.owner,
          'enable loyalty points',
        );

        await submitTx(
          api.tx.entityLoyalty.updatePointsConfig(base.shopId, 800, null, null),
          base.owner,
          'update loyalty reward rate',
        );

        await submitTx(
          api.tx.entityLoyalty.managerIssuePoints(base.shopId, actors.charlie.address, nex(20).toString()),
          base.owner,
          'issue points to charlie',
        );

        await submitTx(
          api.tx.entityLoyalty.transferPoints(base.shopId, actors.dave.address, nex(5).toString()),
          actors.charlie,
          'charlie transfers points to dave',
        );

        const daveBeforeRedeem = await readFreeBalance(actors.dave.address);

        await submitTx(
          api.tx.entityLoyalty.redeemPoints(base.shopId, nex(2).toString()),
          actors.dave,
          'dave redeems points',
        );

        const charliePoints = toStringMaybe(await api.query.entityLoyalty.shopPointsBalances(base.shopId, actors.charlie.address));
        const davePoints = toStringMaybe(await api.query.entityLoyalty.shopPointsBalances(base.shopId, actors.dave.address));
        const daveAfterRedeem = await readFreeBalance(actors.dave.address);

        assert(BigInt(charliePoints) === nex(15), `Expected Charlie points to be 15 NEX-equivalent, got ${charliePoints}`);
        assert(BigInt(davePoints) === nex(3), `Expected Dave points to be 3 NEX-equivalent, got ${davePoints}`);
        assert(daveAfterRedeem > daveBeforeRedeem, 'Dave free balance should increase after redeeming points');

        return {
          charliePoints,
          davePoints,
          daveBeforeRedeem: daveBeforeRedeem.toString(),
          daveAfterRedeem: daveAfterRedeem.toString(),
        };
      });
    },
  );

  await runCase(
    'entity-product-order-physical-flow',
    'Physical product lifecycle + shipping + refund',
    ['pallet-entity-product', 'pallet-entity-order'],
    async (caseEntry) => {
      const base = await ensureBaseContext();

      await reporter.step(caseEntry, 'ensure buyers are funded for physical orders', async () => {
        return {
          charlie: (await ensureNamedBalance('charlie', 5_000)).toString(),
          dave: (await ensureNamedBalance('dave', 5_000)).toString(),
        };
      });

      const productId = await reporter.step(caseEntry, 'create, update, and publish a physical product', async () => {
        const nextProductId = Number((await api.query.entityProduct.nextProductId()).toString());

        await submitTx(
          api.tx.entityProduct.createProduct(
            base.shopId,
            `physical-name-${uniqueSuffix()}`,
            `physical-images-${uniqueSuffix()}`,
            `physical-detail-${uniqueSuffix()}`,
            nex(60).toString(),
            0,
            10,
            'Physical',
            0,
            '',
            '',
            1,
            5,
            'Public',
          ),
          base.owner,
          'create physical product',
        );

        await submitTx(
          api.tx.entityProduct.updateProduct(
            nextProductId,
            `physical-name-updated-${uniqueSuffix()}`,
            null,
            null,
            nex(75).toString(),
            null,
            12,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
          ),
          base.owner,
          'update physical product',
        );

        await submitTx(
          api.tx.entityProduct.publishProduct(nextProductId),
          base.owner,
          'publish physical product',
        );

        const product = await readProduct(nextProductId);
        assert(readField(product, 'status') === 'OnSale', `Expected product status OnSale, got ${readField(product, 'status')}`);
        return {
          productId: nextProductId,
          product,
        };
      }).then((result) => result.productId);

      await reporter.step(caseEntry, 'run the paid → shipping → tracking → confirm receipt flow', async () => {
        const nextOrderId = Number((await api.query.entityTransaction.nextOrderId()).toString());

        await submitTx(
          api.tx.entityTransaction.placeOrder(
            productId,
            1,
            `shipping-${uniqueSuffix()}`,
            null,
            null,
            null,
            `note-${uniqueSuffix()}`,
            null,
          ),
          actors.charlie,
          'charlie place physical order',
        );

        await submitTx(
          api.tx.entityTransaction.updateShippingAddress(nextOrderId, `shipping-updated-${uniqueSuffix()}`),
          actors.charlie,
          'charlie update shipping address',
        );

        await submitTx(
          api.tx.entityTransaction.shipOrder(nextOrderId, `tracking-${uniqueSuffix()}`),
          base.owner,
          'owner ships order',
        );

        await submitTx(
          api.tx.entityTransaction.updateTracking(nextOrderId, `tracking-updated-${uniqueSuffix()}`),
          base.owner,
          'owner updates tracking',
        );

        await submitTx(
          api.tx.entityTransaction.extendConfirmTimeout(nextOrderId),
          actors.charlie,
          'charlie extends confirm timeout',
        );

        await submitTx(
          api.tx.entityTransaction.confirmReceipt(nextOrderId),
          actors.charlie,
          'charlie confirms receipt',
        );

        const order = await readOrder(nextOrderId);
        assert(readField(order, 'status') === 'Completed', `Expected completed order, got ${readField(order, 'status')}`);
        return {
          orderId: nextOrderId,
          order,
        };
      });

      await reporter.step(caseEntry, 'run the refund dispute flow on a second physical order', async () => {
        const nextOrderId = Number((await api.query.entityTransaction.nextOrderId()).toString());

        await submitTx(
          api.tx.entityTransaction.placeOrder(
            productId,
            1,
            `shipping-dave-${uniqueSuffix()}`,
            null,
            null,
            null,
            `note-dave-${uniqueSuffix()}`,
            null,
          ),
          actors.dave,
          'dave place physical order',
        );

        await submitTx(
          api.tx.entityTransaction.requestRefund(nextOrderId, `refund-${uniqueSuffix()}`),
          actors.dave,
          'dave requests refund',
        );

        const disputed = await readOrder(nextOrderId);
        assert(readField(disputed, 'status') === 'Disputed', `Expected disputed order, got ${readField(disputed, 'status')}`);

        await submitTx(
          api.tx.entityTransaction.approveRefund(nextOrderId),
          base.owner,
          'owner approves refund',
        );

        const refunded = await readOrder(nextOrderId);
        assert(readField(refunded, 'status') === 'Refunded', `Expected refunded order, got ${readField(refunded, 'status')}`);

        await submitTx(
          api.tx.entityProduct.unpublishProduct(productId),
          base.owner,
          'unpublish physical product after flow',
        );

        const product = await readProduct(productId);
        assert(
          ['Draft', 'OffShelf'].includes(readField(product, 'status')),
          `Expected unpublished product to return to Draft or OffShelf, got ${readField(product, 'status')}`,
        );

        return {
          orderId: nextOrderId,
          refunded,
          productStatusAfterUnpublish: readField(product, 'status'),
        };
      });
    },
  );

  await runCase(
    'commission-admin-controls',
    'Commission plugin control-plane flows',
    [
      'pallet-commission-single-line',
      'pallet-commission-multi-level',
      'pallet-commission-pool-reward',
    ],
    async (caseEntry) => {
      const base = await ensureBaseContext();

      await reporter.step(caseEntry, 'configure single-line, update params, pause/resume, and schedule a pending config', async () => {
        await submitTx(
          api.tx.commissionSingleLine.setSingleLineConfig(base.entityId, 100, 100, 3, 3, 0, 4, 4),
          base.owner,
          'set single-line config',
        );

        await submitTx(
          api.tx.commissionSingleLine.updateSingleLineParams(base.entityId, 120, 150, null, null, null, null, null),
          base.owner,
          'update single-line params',
        );

        await submitTx(
          api.tx.commissionSingleLine.pauseSingleLine(base.entityId),
          base.owner,
          'pause single-line',
        );

        await submitTx(
          api.tx.commissionSingleLine.resumeSingleLine(base.entityId),
          base.owner,
          'resume single-line',
        );

        await submitTx(
          api.tx.commissionSingleLine.scheduleConfigChange(base.entityId, 130, 140, 3, 3, 0, 5, 5),
          base.owner,
          'schedule single-line config change',
        );

        const config = toJson(await api.query.commissionSingleLine.singleLineConfigs(base.entityId));
        const pending = toJson(await api.query.commissionSingleLine.pendingConfigChanges(base.entityId));
        const enabled = toJson(await api.query.commissionSingleLine.singleLineEnabled(base.entityId));
        assert(config != null, 'single-line config should exist');
        assert(pending != null, 'single-line pending config should exist');

        return {
          enabled,
          config,
          pending,
        };
      });

      await reporter.step(caseEntry, 'configure multi-level, add/remove a tier, pause/resume, and schedule a pending config', async () => {
        const baseTiers = [
          { rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 },
          { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 },
        ];

        await submitTx(
          api.tx.commissionMultiLevel.setMultiLevelConfig(base.entityId, baseTiers, 300),
          base.owner,
          'set multi-level config',
        );

        await submitTx(
          api.tx.commissionMultiLevel.addTier(
            base.entityId,
            2,
            { rate: 50, required_directs: 0, required_team_size: 0, required_spent: 0 },
          ),
          base.owner,
          'add multi-level tier',
        );

        await submitTx(
          api.tx.commissionMultiLevel.removeTier(base.entityId, 2),
          base.owner,
          'remove multi-level tier',
        );

        await submitTx(
          api.tx.commissionMultiLevel.pauseMultiLevel(base.entityId),
          base.owner,
          'pause multi-level',
        );

        await submitTx(
          api.tx.commissionMultiLevel.resumeMultiLevel(base.entityId),
          base.owner,
          'resume multi-level',
        );

        await submitTx(
          api.tx.commissionMultiLevel.scheduleConfigChange(
            base.entityId,
            [
              { rate: 220, required_directs: 0, required_team_size: 0, required_spent: 0 },
              { rate: 80, required_directs: 0, required_team_size: 0, required_spent: 0 },
            ],
            320,
          ),
          base.owner,
          'schedule multi-level config change',
        );

        const config = toJson(await api.query.commissionMultiLevel.multiLevelConfigs(base.entityId));
        const pending = toJson(await api.query.commissionMultiLevel.pendingConfigs(base.entityId));
        assert(config != null, 'multi-level config should exist');
        assert(pending != null, 'multi-level pending config should exist');

        return {
          config,
          pending,
        };
      });

      await reporter.step(caseEntry, 'configure pool reward, pause/resume, and schedule a pending config change', async () => {
        await submitTx(
          api.tx.commissionPoolReward.setPoolRewardConfig(base.entityId, [[0, 10_000]], 14_400),
          base.owner,
          'set pool reward config',
        );

        await submitTx(
          api.tx.commissionPoolReward.pausePoolReward(base.entityId),
          base.owner,
          'pause pool reward',
        );

        await submitTx(
          api.tx.commissionPoolReward.resumePoolReward(base.entityId),
          base.owner,
          'resume pool reward',
        );

        await submitTx(
          api.tx.commissionPoolReward.schedulePoolRewardConfigChange(base.entityId, [[0, 9_000], [1, 1_000]], 14_400),
          base.owner,
          'schedule pool reward config change',
        );

        const config = toJson(await api.query.commissionPoolReward.poolRewardConfigs(base.entityId));
        const pending = toJson(await api.query.commissionPoolReward.pendingPoolRewardConfig(base.entityId));
        const paused = toJson(await api.query.commissionPoolReward.poolRewardPaused(base.entityId));
        assert(config != null, 'pool reward config should exist');
        assert(pending != null, 'pool reward pending config should exist');

        return {
          paused,
          config,
          pending,
        };
      });
    },
  );

  await runCase(
    'nex-market-trade-flow',
    'Matched sell order → reserve → payment confirmation → seller settlement',
    ['pallet-nex-market'],
    async (caseEntry) => {
      await reporter.step(caseEntry, 'ensure the buyer has enough free balance for the remote market deposit model', async () => {
        const charlieBalance = await ensureNamedBalance('charlie', 65_000);
        return {
          charlieFree: charlieBalance.toString(),
          note: 'Remote node priceProtection.initialPrice=10, so buyer min deposit is unusually large.',
        };
      });

      const marketPrice = await reporter.step(caseEntry, 'capture the current market price protection snapshot', async () => {
        const priceProtection = toJson(await api.query.nexMarket.priceProtectionStore());
        const currentPrice = Number(readField(priceProtection, 'initialPrice', 'initial_price') ?? 500_000);
        assert(currentPrice > 0, 'market price should be positive');
        return {
          currentPrice,
          priceProtection,
        };
      }).then((result) => result.currentPrice);

      const orderId = await reporter.step(caseEntry, 'place a sell order with a validated TRON address', async () => {
        const nextOrderId = Number((await api.query.nexMarket.nextOrderId()).toString());
        await submitTx(
          api.tx.nexMarket.placeSellOrder(nex(10).toString(), marketPrice, VALID_TRON_ADDRESSES.seller, null),
          actors.bob,
          'place sell order',
        );

        const order = toJson(await api.query.nexMarket.orders(nextOrderId));
        assert(order != null, 'sell order should exist after placement');
        assert(readField(order, 'status') === 'Open', `Expected open sell order, got ${readField(order, 'status')}`);
        return {
          orderId: nextOrderId,
          order,
        };
      }).then((result) => result.orderId);

      const tradeId = await reporter.step(caseEntry, 'reserve the sell order and lock the buyer deposit', async () => {
        const nextTradeId = Number((await api.query.nexMarket.nextUsdtTradeId()).toString());

        await submitTx(
          api.tx.nexMarket.reserveSellOrder(orderId, null, VALID_TRON_ADDRESSES.buyer),
          actors.charlie,
          'reserve sell order',
        );

        const trade = toJson(await api.query.nexMarket.usdtTrades(nextTradeId));
        assert(trade != null, 'trade should exist after reserving the sell order');
        assert(readField(trade, 'status') === 'AwaitingPayment', `Expected AwaitingPayment, got ${readField(trade, 'status')}`);

        return {
          tradeId: nextTradeId,
          trade,
          buyerDeposit: readField(trade, 'buyerDeposit', 'buyer_deposit'),
        };
      }).then((result) => result.tradeId);

      await reporter.step(caseEntry, 'confirm payment and settle the trade', async () => {
        await submitTx(
          api.tx.nexMarket.confirmPayment(tradeId),
          actors.charlie,
          'confirm market payment',
        );

        await submitTx(
          api.tx.nexMarket.sellerConfirmReceived(tradeId),
          actors.bob,
          'seller confirm received',
        );

        const trade = toJson(await api.query.nexMarket.usdtTrades(tradeId));
        const order = toJson(await api.query.nexMarket.orders(orderId));
        assert(readField(trade, 'status') === 'Completed', `Expected completed trade, got ${readField(trade, 'status')}`);
        assert(readField(order, 'status') === 'Filled', `Expected filled order, got ${readField(order, 'status')}`);

        return {
          trade,
          order,
        };
      });
    },
  );

  reporter.results.meta.endedAt = new Date().toISOString();
  reporter.results.meta.baseEntityContext = state.baseContext ? {
    ownerName: state.baseContext.ownerName,
    entityId: state.baseContext.entityId,
    primaryShopId: state.baseContext.shopId,
    secondaryShopId: state.baseContext.shopId,
  } : null;

  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const jsonPath = path.join(ARTIFACT_DIR, `run-${timestamp}.json`);
  await reporter.finalize();
  await fs.writeFile(jsonPath, JSON.stringify(jsonSafe(reporter.results), null, 2));

  const markdown = buildMarkdownReport(reporter.results);
  await fs.writeFile(REPORT_PATH, markdown);
  await writeJsonAtomic(LATEST_JSON_PATH, reporter.results);
  await writeJsonAtomic(EXECUTION_STATUS_PATH, {
    date: REPORT_DATE,
    wsUrl: WS_URL,
    selectedCases: CLI.selectedCases,
    summary: reporter.results.summary,
    cases: reporter.results.cases.map((caseEntry) => ({
      id: caseEntry.id,
      title: caseEntry.title,
      status: caseEntry.status,
      modules: caseEntry.modules,
      startedAt: caseEntry.startedAt,
      endedAt: caseEntry.endedAt,
      error: caseEntry.error,
    })),
    latestJsonPath: LATEST_JSON_PATH,
    reportPath: REPORT_PATH,
    runJsonPath: jsonPath,
    updatedAt: reporter.results.meta.endedAt,
  });
  await writeBootstrapStatus('completed', {
    summary: reporter.results.summary,
    resultJsonPath: jsonPath,
  });

  await reporter.log(`\nArtifacts written:`);
  await reporter.log(`- ${REPORT_PATH}`);
  await reporter.log(`- ${jsonPath}`);
  await reporter.log(`- ${LATEST_JSON_PATH}`);
  await reporter.log(`- ${EXECUTION_STATUS_PATH}`);

  await api.disconnect();

  if (reporter.results.summary.failed > 0) {
    process.exitCode = 1;
  }
}

function buildMarkdownReport(results) {
  const lines = [];
  lines.push(`# 远程业务流测试报告`);
  lines.push('');
  lines.push(`- 日期：${results.meta.reportDate}`);
  lines.push(`- 节点：\`${results.meta.wsUrl}\``);
  lines.push(`- 链：${results.meta.chain}`);
  lines.push(`- 节点版本：${results.meta.nodeName} ${results.meta.nodeVersion}`);
  lines.push(`- Runtime：${results.meta.specName} v${results.meta.specVersion}`);
  lines.push(`- API：\`@polkadot/api ${results.meta.apiVersion}\`（来源：\`${results.meta.apiRoot}\`）`);
  if (Array.isArray(results.meta.selectedCases) && results.meta.selectedCases.length > 0) {
    lines.push(`- 选择执行：${results.meta.selectedCases.map((item) => `\`${item}\``).join('、')}`);
  }
  lines.push(`- 开始：${results.meta.startedAt}`);
  lines.push(`- 结束：${results.meta.endedAt}`);
  lines.push('');

  lines.push(`## 总结`);
  lines.push('');
  lines.push(`- 通过：**${results.summary.passed}**`);
  lines.push(`- 失败：**${results.summary.failed}**`);
  lines.push(`- 跳过：**${results.summary.skipped}**`);
  if (results.meta.baseEntityContext) {
    lines.push(`- 本次实体上下文：owner=\`${results.meta.baseEntityContext.ownerName}\`, entity=\`${results.meta.baseEntityContext.entityId}\`, primary_shop=\`${results.meta.baseEntityContext.primaryShopId}\``);
  }
  lines.push('');

  lines.push(`## 已跳过的既有流`);
  lines.push('');
  lines.push(`> 按用户要求，以下流不重复执行；它们已经存在于仓库的现有 E2E 套件中。`);
  lines.push('');
  lines.push(`| 模块 | 已有流 | 现有套件 | 原因 |`);
  lines.push(`|---|---|---|---|`);
  for (const item of results.skippedAlreadyCovered) {
    lines.push(`| ${item.module} | ${item.flow} | \`${item.existingSuite}\` | ${item.reason} |`);
  }
  lines.push('');

  lines.push(`## 本次新增执行的远程流`);
  lines.push('');
  for (const caseEntry of results.cases) {
    lines.push(`### ${caseEntry.title}`);
    lines.push('');
    lines.push(`- ID：\`${caseEntry.id}\``);
    lines.push(`- 模块：${caseEntry.modules.map((item) => `\`${item}\``).join('、')}`);
    lines.push(`- 状态：**${caseEntry.status}**`);
    if (caseEntry.error) {
      lines.push(`- 错误：${caseEntry.error}`);
    }
    if (caseEntry.notes.length > 0) {
      lines.push(`- 备注：${caseEntry.notes.join('；')}`);
    }
    lines.push('');
    lines.push(`| 步骤 | 状态 | 耗时(ms) | 关键信息 |`);
    lines.push(`|---|---:|---:|---|`);
    for (const step of caseEntry.steps) {
      const info = step.status === 'passed'
        ? summarizeStepOutput(step.output)
        : step.error;
      lines.push(`| ${step.title} | ${step.status} | ${step.durationMs} | ${escapePipes(info)} |`);
    }
    lines.push('');
  }

  lines.push(`## 结论`);
  lines.push('');

  if (results.summary.failed === 0) {
    lines.push(`本次针对用户点名的模块，已在远程节点 \`${results.meta.wsUrl}\` 上完成**未被现有套件覆盖**的业务流验证：`);
    lines.push('');
    lines.push(`- \`pallet-entity-shop\`：二级店铺创建、经理增删、主店切换、运营资金充值/提取`);
    lines.push(`- \`pallet-entity-member\`：待审批入会、批量审批`);
    lines.push(`- \`pallet-entity-loyalty\`：积分启用、配置更新、发放、转移、兑换`);
    lines.push(`- \`pallet-entity-product\`：实体商品创建、更新、发布、下架`);
    lines.push(`- \`pallet-entity-order\`：实物订单支付、改地址、发货、改物流、延长确认、确认收货、退款审批`);
    lines.push(`- \`pallet-commission-single-line\`：参数更新、暂停/恢复、延迟配置挂起`);
    lines.push(`- \`pallet-commission-multi-level\`：层级增删、暂停/恢复、延迟配置挂起`);
    lines.push(`- \`pallet-commission-pool-reward\`：配置更新、暂停/恢复、延迟配置挂起`);
    lines.push(`- \`pallet-nex-market\`：卖单撮合、买家保证金锁定、确认付款、卖家确认收款、成交完成`);
    lines.push('');
    lines.push(`需要注意的是，当前远程节点的 \`priceProtection.initialPrice = 10\`，会把 \`nex-market\` 买家最小保证金抬高到异常水平，因此测试里额外为买家补足了更高的 NEX 余额。`);
  } else {
    lines.push(`本次仍有失败项，请结合 \`artifacts/latest.json\` 查看失败步骤与错误信息。`);
  }

  lines.push('');
  return lines.join('\n');
}

function summarizeStepOutput(output) {
  if (output == null) {
    return '';
  }
  if (typeof output === 'string' || typeof output === 'number' || typeof output === 'boolean') {
    return String(output);
  }
  if (Array.isArray(output)) {
    return JSON.stringify(output);
  }
  if (typeof output === 'object') {
    const preview = [];
    for (const [key, value] of Object.entries(output).slice(0, 6)) {
      if (typeof value === 'object' && value !== null) {
        preview.push(`${key}=${JSON.stringify(value).slice(0, 100)}`);
      } else {
        preview.push(`${key}=${String(value)}`);
      }
    }
    return preview.join('; ');
  }
  return String(output);
}

function escapePipes(text) {
  return String(text ?? '').replace(/\|/g, '\\|').replace(/\n/g, '<br>');
}

main().catch(async (error) => {
  const message = `Fatal runner error: ${error instanceof Error ? error.stack ?? error.message : String(error)}`;
  console.error(message);
  try {
    await writeBootstrapStatus('fatal', {
      error: error instanceof Error ? error.message : String(error),
    });
    await appendLine(BOOTSTRAP_LOG_PATH, `[${new Date().toISOString()}] ${message}`);
  } catch {
    // ignore secondary persistence failures during fatal handling
  }
  process.exitCode = 1;
});
