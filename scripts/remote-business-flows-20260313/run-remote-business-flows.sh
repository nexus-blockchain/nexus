#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARTIFACT_DIR="$SCRIPT_DIR/artifacts"
WS_URL="${WS_URL:-wss://202.140.140.202}"

mkdir -p "$ARTIFACT_DIR"

ALL_CASES=(
  "entity-shop-flow"
  "entity-member-loyalty-flow"
  "entity-product-order-physical-flow"
  "commission-admin-controls"
  "nex-market-trade-flow"
)

SELECTED_CASES=()

print_list() {
  cat <<'EOF'
entity-shop-flow	Entity shop extended lifecycle	pallet-entity-shop
entity-member-loyalty-flow	Approval onboarding + points issue/transfer/redeem	pallet-entity-member, pallet-entity-loyalty
entity-product-order-physical-flow	Physical product lifecycle + shipping + refund	pallet-entity-product, pallet-entity-order
commission-admin-controls	Commission plugin control-plane flows	pallet-commission-single-line, pallet-commission-multi-level, pallet-commission-pool-reward
nex-market-trade-flow	Matched sell order → reserve → payment confirmation → seller settlement	pallet-nex-market
EOF
}

usage() {
  cat <<'EOF'
Usage: bash remote-business-flows-20260313/run-remote-business-flows.sh [--list] [--case <id[,id...]>]

Options:
  --list              List runnable cases
  --case <ids>        Run only the specified case ids (comma-separated or repeatable)
  --help              Show this help
EOF
}

contains_case() {
  local wanted="$1"
  shift
  local current
  for current in "$@"; do
    [[ "$current" == "$wanted" ]] && return 0
  done
  return 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --list)
      print_list
      exit 0
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --case)
      shift
      [[ $# -gt 0 ]] || { echo "Missing value after --case" >&2; exit 1; }
      IFS=',' read -r -a _parts <<<"$1"
      for _part in "${_parts[@]}"; do
        [[ -n "$_part" ]] && SELECTED_CASES+=("$_part")
      done
      ;;
    --case=*)
      IFS=',' read -r -a _parts <<<"${1#--case=}"
      for _part in "${_parts[@]}"; do
        [[ -n "$_part" ]] && SELECTED_CASES+=("$_part")
      done
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ ${#SELECTED_CASES[@]} -eq 0 ]] && [[ -n "${REMOTE_FLOW_CASES:-}" ]]; then
  IFS=',' read -r -a SELECTED_CASES <<<"$REMOTE_FLOW_CASES"
fi

if [[ ${#SELECTED_CASES[@]} -gt 0 ]]; then
  for _selected in "${SELECTED_CASES[@]}"; do
    contains_case "$_selected" "${ALL_CASES[@]}" || {
      echo "Unknown case id: $_selected" >&2
      exit 1
    }
  done
fi

case_selected() {
  local case_id="$1"
  if [[ ${#SELECTED_CASES[@]} -eq 0 ]]; then
    return 0
  fi
  contains_case "$case_id" "${SELECTED_CASES[@]}"
}

export WS_URL
export REMOTE_BF_ARTIFACT_DIR="$ARTIFACT_DIR"
export NODE_TLS_REJECT_UNAUTHORIZED="${NODE_TLS_REJECT_UNAUTHORIZED:-0}"
export POLKADOTJS_DISABLE_ESM_CJS_WARNING="${POLKADOTJS_DISABLE_ESM_CJS_WARNING:-1}"

echo "[remote-business-flows] ws=$WS_URL"
if [[ ${#SELECTED_CASES[@]} -gt 0 ]]; then
  echo "[remote-business-flows] selected=${SELECTED_CASES[*]}"
fi

if case_selected "entity-shop-flow"; then
  echo
  echo "==> entity-shop-flow"
  node --input-type=module <<'EOFJS'
import fs from 'node:fs/promises';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_URL = process.env.WS_URL;
const ARTIFACT_DIR = process.env.REMOTE_BF_ARTIFACT_DIR;
const NEX = 1_000_000_000_000n;

function toJson(v) { return v && typeof v.toJSON === 'function' ? v.toJSON() : v; }
function readField(obj, ...keys) {
  if (!obj || typeof obj !== 'object') return undefined;
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) return obj[key];
  }
  return undefined;
}
function safe(v) { return JSON.parse(JSON.stringify(v, (_, cur) => typeof cur === 'bigint' ? cur.toString() : cur)); }
function unique() { return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`; }
function decodeError(api, dispatchError) {
  if (!dispatchError) return null;
  if (dispatchError.isModule) {
    const meta = api.registry.findMetaError(dispatchError.asModule);
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
  }
  return dispatchError.toString();
}
async function submit(api, tx, signer, label) {
  return await new Promise((resolve, reject) => {
    let unsub = () => {};
    const timer = setTimeout(() => reject(new Error(`timeout ${label}`)), 240000);
    tx.signAndSend(signer, (result) => {
      if (!result.status.isFinalized) return;
      clearTimeout(timer);
      try { unsub(); } catch {}
      const err = decodeError(api, result.dispatchError);
      if (err) return reject(new Error(`${label}: ${err}`));
      resolve({
        txHash: tx.hash.toHex(),
        status: result.status.type,
        events: result.events.map((r) => ({
          name: `${r.event.section}.${r.event.method}`,
          data: toJson(r.event.data),
        })),
      });
    }).then((u) => { unsub = u; }).catch((e) => {
      clearTimeout(timer);
      reject(e);
    });
  });
}

await cryptoWaitReady();
const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
const keyring = new Keyring({ type: 'sr25519', ss58Format: 273 });
const actors = {
  alice: keyring.addFromUri('//Alice'),
  bob: keyring.addFromUri('//Bob'),
  charlie: keyring.addFromUri('//Charlie'),
  dave: keyring.addFromUri('//Dave'),
  eve: keyring.addFromUri('//Eve'),
  ferdie: keyring.addFromUri('//Ferdie'),
};

let ownerName = null;
let owner = null;
for (const candidate of ['alice', 'dave', 'ferdie', 'charlie', 'bob']) {
  const entityIds = toJson(await api.query.entityRegistry.userEntity(actors[candidate].address));
  if (Array.isArray(entityIds) && entityIds.length < 3) {
    ownerName = candidate;
    owner = actors[candidate];
    break;
  }
}
if (!owner) throw new Error('No dev actor has remaining entity capacity');

const nextEntityId = Number((await api.query.entityRegistry.nextEntityId()).toString());
const createEntityReceipt = await submit(api, api.tx.entityRegistry.createEntity(`rbf-${unique()}`, null, null, null), owner, 'createEntity');
const entityAfterCreate = toJson((await api.query.entityRegistry.entities(nextEntityId)).unwrap());
const initialPrimaryShopId = Number(readField(entityAfterCreate, 'primaryShopId', 'primary_shop_id'));
const nextShopId = Number((await api.query.entityShop.nextShopId()).toString());

const receipts = {
  createEntity: createEntityReceipt,
  createShop: await submit(api, api.tx.entityShop.createShop(nextEntityId, `rbf-shop-${unique()}`, 'OnlineStore', 0), owner, 'createShop'),
  addManager: await submit(api, api.tx.entityShop.addManager(nextShopId, actors.bob.address), owner, 'addManager'),
  updateShop: await submit(api, api.tx.entityShop.updateShop(nextShopId, `rbf-shop-updated-${unique()}`, null, null, null, null), actors.bob, 'updateShop'),
  setPrimaryShop: await submit(api, api.tx.entityShop.setPrimaryShop(nextEntityId, nextShopId), owner, 'setPrimaryShop'),
  fundOperating: await submit(api, api.tx.entityShop.fundOperating(nextShopId, (5000n * NEX).toString()), owner, 'fundOperating'),
  withdrawOperatingFund: await submit(api, api.tx.entityShop.withdrawOperatingFund(nextShopId, (50n * NEX).toString()), owner, 'withdrawOperatingFund'),
  removeManager: await submit(api, api.tx.entityShop.removeManager(nextShopId, actors.bob.address), owner, 'removeManager'),
};

const finalEntity = toJson((await api.query.entityRegistry.entities(nextEntityId)).unwrap());
const finalShop = toJson((await api.query.entityShop.shops(nextShopId)).unwrap());
const result = {
  case: 'entity-shop-flow',
  passed: true,
  timestamp: new Date().toISOString(),
  ownerName,
  ownerAddress: owner.address,
  entityId: nextEntityId,
  initialPrimaryShopId,
  shopId: nextShopId,
  receipts,
  finalEntity,
  finalShop,
  observations: {
    primaryShopId: readField(finalEntity, 'primaryShopId', 'primary_shop_id'),
    managers: readField(finalShop, 'managers') ?? [],
    nameHex: finalShop.name,
  },
};

await fs.writeFile(`${ARTIFACT_DIR}/entity-shop-flow.json`, JSON.stringify(safe(result), null, 2));
await fs.writeFile(`${ARTIFACT_DIR}/base-context.json`, JSON.stringify(safe({
  ownerName,
  ownerAddress: owner.address,
  entityId: nextEntityId,
  primaryShopId: readField(finalEntity, 'primaryShopId', 'primary_shop_id'),
  secondaryShopId: nextShopId,
  createdAt: result.timestamp,
}), null, 2));

console.log(JSON.stringify({
  case: result.case,
  ownerName,
  entityId: nextEntityId,
  shopId: nextShopId,
  primaryShopId: result.observations.primaryShopId,
}, null, 2));

await api.disconnect();
EOFJS
fi

if case_selected "entity-member-loyalty-flow"; then
  echo
  echo "==> entity-member-loyalty-flow"
  node --input-type=module <<'EOFJS'
import fs from 'node:fs/promises';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_URL = process.env.WS_URL;
const ARTIFACT_DIR = process.env.REMOTE_BF_ARTIFACT_DIR;
const NEX = 1_000_000_000_000n;

function toJson(v) { return v && typeof v.toJSON === 'function' ? v.toJSON() : v; }
function safe(v) { return JSON.parse(JSON.stringify(v, (_, cur) => typeof cur === 'bigint' ? cur.toString() : cur)); }
function decodeError(api, dispatchError) {
  if (!dispatchError) return null;
  if (dispatchError.isModule) {
    const meta = api.registry.findMetaError(dispatchError.asModule);
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
  }
  return dispatchError.toString();
}
async function submit(api, tx, signer, label) {
  return await new Promise((resolve, reject) => {
    let unsub = () => {};
    const timer = setTimeout(() => reject(new Error(`timeout ${label}`)), 240000);
    tx.signAndSend(signer, (result) => {
      if (!result.status.isFinalized) return;
      clearTimeout(timer);
      try { unsub(); } catch {}
      const err = decodeError(api, result.dispatchError);
      if (err) return reject(new Error(`${label}: ${err}`));
      resolve(result.events.map((r) => ({
        name: `${r.event.section}.${r.event.method}`,
        data: toJson(r.event.data),
      })));
    }).then((u) => { unsub = u; }).catch((e) => {
      clearTimeout(timer);
      reject(e);
    });
  });
}
async function free(api, address) {
  return BigInt((await api.query.system.account(address)).data.free.toString());
}
async function ensureBalance(api, alice, target, minimum) {
  const current = await free(api, target.address);
  if (current >= minimum) return current;
  await submit(api, api.tx.balances.transferKeepAlive(target.address, (minimum - current).toString()), alice, `fund-${target.address}`);
  return free(api, target.address);
}

const base = JSON.parse(await fs.readFile(`${ARTIFACT_DIR}/base-context.json`, 'utf8'));
await cryptoWaitReady();
const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
const keyring = new Keyring({ type: 'sr25519', ss58Format: 273 });
const actors = {
  alice: keyring.addFromUri('//Alice'),
  bob: keyring.addFromUri('//Bob'),
  charlie: keyring.addFromUri('//Charlie'),
  dave: keyring.addFromUri('//Dave'),
  eve: keyring.addFromUri('//Eve'),
  ferdie: keyring.addFromUri('//Ferdie'),
};
const owner = actors[base.ownerName];
if (!owner) throw new Error(`Unknown ownerName ${base.ownerName}`);

await ensureBalance(api, actors.alice, actors.charlie, 5000n * NEX);
await ensureBalance(api, actors.alice, actors.dave, 5000n * NEX);

const artifact = {
  case: 'entity-member-loyalty-flow',
  passed: false,
  entityId: base.entityId,
  shopId: base.secondaryShopId,
  steps: [],
};

artifact.steps.push({ step: 'setMemberPolicy', events: await submit(api, api.tx.entityMember.setMemberPolicy(base.secondaryShopId, 4), owner, 'setMemberPolicy') });
artifact.steps.push({ step: 'registerCharlie', events: await submit(api, api.tx.entityMember.registerMember(base.secondaryShopId, null), actors.charlie, 'registerCharlie') });
artifact.steps.push({ step: 'registerDave', events: await submit(api, api.tx.entityMember.registerMember(base.secondaryShopId, null), actors.dave, 'registerDave') });
artifact.pending = {
  charlie: toJson(await api.query.entityMember.pendingMembers(base.entityId, actors.charlie.address)),
  dave: toJson(await api.query.entityMember.pendingMembers(base.entityId, actors.dave.address)),
};
artifact.steps.push({ step: 'batchApproveMembers', events: await submit(api, api.tx.entityMember.batchApproveMembers(base.secondaryShopId, [actors.charlie.address, actors.dave.address]), owner, 'batchApproveMembers') });
artifact.members = {
  charlie: toJson(await api.query.entityMember.entityMembers(base.entityId, actors.charlie.address)),
  dave: toJson(await api.query.entityMember.entityMembers(base.entityId, actors.dave.address)),
  memberCount: (await api.query.entityMember.memberCount(base.entityId)).toString(),
};

artifact.steps.push({ step: 'enablePoints', events: await submit(api, api.tx.entityLoyalty.enablePoints(base.secondaryShopId, 'RemoteFlowPts', 'RFP', 500, 10000, true), owner, 'enablePoints') });
artifact.steps.push({ step: 'updatePointsConfig', events: await submit(api, api.tx.entityLoyalty.updatePointsConfig(base.secondaryShopId, 800, null, null), owner, 'updatePointsConfig') });
artifact.steps.push({ step: 'managerIssuePoints', events: await submit(api, api.tx.entityLoyalty.managerIssuePoints(base.secondaryShopId, actors.charlie.address, (20n * NEX).toString()), owner, 'managerIssuePoints') });
artifact.steps.push({ step: 'transferPoints', events: await submit(api, api.tx.entityLoyalty.transferPoints(base.secondaryShopId, actors.dave.address, (5n * NEX).toString()), actors.charlie, 'transferPoints') });

const daveFreeBefore = (await api.query.system.account(actors.dave.address)).data.free.toString();
const daveShoppingBefore = (await api.query.entityLoyalty.memberShoppingBalance(base.entityId, actors.dave.address)).toString();
artifact.steps.push({ step: 'redeemPoints', events: await submit(api, api.tx.entityLoyalty.redeemPoints(base.secondaryShopId, (2n * NEX).toString()), actors.dave, 'redeemPoints') });
const daveFreeAfter = (await api.query.system.account(actors.dave.address)).data.free.toString();
const daveShoppingAfter = (await api.query.entityLoyalty.memberShoppingBalance(base.entityId, actors.dave.address)).toString();

artifact.points = {
  config: toJson(await api.query.entityLoyalty.shopPointsConfigs(base.secondaryShopId)),
  charlie: (await api.query.entityLoyalty.shopPointsBalances(base.secondaryShopId, actors.charlie.address)).toString(),
  dave: (await api.query.entityLoyalty.shopPointsBalances(base.secondaryShopId, actors.dave.address)).toString(),
  totalSupply: (await api.query.entityLoyalty.shopPointsTotalSupply(base.secondaryShopId)).toString(),
  daveFreeBefore,
  daveFreeAfter,
  daveShoppingBefore,
  daveShoppingAfter,
};

artifact.passed = true;
await fs.writeFile(`${ARTIFACT_DIR}/entity-member-loyalty-flow.json`, JSON.stringify(safe(artifact), null, 2));
console.log(JSON.stringify({
  case: artifact.case,
  entityId: artifact.entityId,
  shopId: artifact.shopId,
  memberCount: artifact.members.memberCount,
  charliePoints: artifact.points.charlie,
  davePoints: artifact.points.dave,
}, null, 2));
await api.disconnect();
EOFJS
fi

if case_selected "entity-product-order-physical-flow"; then
  echo
  echo "==> entity-product-order-physical-flow"
  node --input-type=module <<'EOFJS'
import fs from 'node:fs/promises';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_URL = process.env.WS_URL;
const ARTIFACT_DIR = process.env.REMOTE_BF_ARTIFACT_DIR;
const NEX = 1_000_000_000_000n;

function toJson(v) { return v && typeof v.toJSON === 'function' ? v.toJSON() : v; }
function safe(v) { return JSON.parse(JSON.stringify(v, (_, cur) => typeof cur === 'bigint' ? cur.toString() : cur)); }
function unique() { return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`; }
function decodeError(api, dispatchError) {
  if (!dispatchError) return null;
  if (dispatchError.isModule) {
    const meta = api.registry.findMetaError(dispatchError.asModule);
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
  }
  return dispatchError.toString();
}
async function submit(api, tx, signer, label) {
  return await new Promise((resolve, reject) => {
    let unsub = () => {};
    const timer = setTimeout(() => reject(new Error(`timeout ${label}`)), 240000);
    tx.signAndSend(signer, (result) => {
      if (!result.status.isFinalized) return;
      clearTimeout(timer);
      try { unsub(); } catch {}
      const err = decodeError(api, result.dispatchError);
      if (err) return reject(new Error(`${label}: ${err}`));
      resolve(result.events.map((r) => ({
        name: `${r.event.section}.${r.event.method}`,
        data: toJson(r.event.data),
      })));
    }).then((u) => { unsub = u; }).catch((e) => {
      clearTimeout(timer);
      reject(e);
    });
  });
}
async function free(api, address) {
  return BigInt((await api.query.system.account(address)).data.free.toString());
}
async function ensureBalance(api, alice, target, minimum) {
  const current = await free(api, target.address);
  if (current >= minimum) return current;
  await submit(api, api.tx.balances.transferKeepAlive(target.address, (minimum - current).toString()), alice, `fund-${target.address}`);
  return free(api, target.address);
}

const base = JSON.parse(await fs.readFile(`${ARTIFACT_DIR}/base-context.json`, 'utf8'));
await cryptoWaitReady();
const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
const keyring = new Keyring({ type: 'sr25519', ss58Format: 273 });
const actors = {
  alice: keyring.addFromUri('//Alice'),
  charlie: keyring.addFromUri('//Charlie'),
  dave: keyring.addFromUri('//Dave'),
  [base.ownerName]: keyring.addFromUri(`//${base.ownerName[0].toUpperCase()}${base.ownerName.slice(1)}`),
};
const owner = actors[base.ownerName];
if (!owner) throw new Error(`Unknown ownerName ${base.ownerName}`);

await ensureBalance(api, actors.alice, actors.charlie, 5000n * NEX);
await ensureBalance(api, actors.alice, actors.dave, 5000n * NEX);

const artifact = {
  case: 'entity-product-order-physical-flow',
  passed: false,
  entityId: base.entityId,
  shopId: base.secondaryShopId,
  steps: [],
};

const productId = Number((await api.query.entityProduct.nextProductId()).toString());
artifact.steps.push({ step: 'createProduct', events: await submit(api, api.tx.entityProduct.createProduct(base.secondaryShopId, `physical-name-${unique()}`, `physical-images-${unique()}`, `physical-detail-${unique()}`, (60n * NEX).toString(), 0, 12, 'Physical', 0, '', '', 1, 5, 'Public'), owner, 'createProduct') });
artifact.steps.push({ step: 'updateProduct', events: await submit(api, api.tx.entityProduct.updateProduct(productId, `physical-name-updated-${unique()}`, null, null, (75n * NEX).toString(), null, 12, null, null, null, null, null, null, null), owner, 'updateProduct') });
artifact.steps.push({ step: 'publishProduct', events: await submit(api, api.tx.entityProduct.publishProduct(productId), owner, 'publishProduct') });
artifact.productAfterPublish = toJson((await api.query.entityProduct.products(productId)).unwrap());

const orderId1 = Number((await api.query.entityTransaction.nextOrderId()).toString());
artifact.steps.push({ step: 'placeOrder1', events: await submit(api, api.tx.entityTransaction.placeOrder(productId, 1, `shipping-${unique()}`, null, null, null, `note-${unique()}`, null), actors.charlie, 'placeOrder1') });
artifact.steps.push({ step: 'updateShippingAddress', events: await submit(api, api.tx.entityTransaction.updateShippingAddress(orderId1, `shipping-updated-${unique()}`), actors.charlie, 'updateShippingAddress') });
artifact.steps.push({ step: 'shipOrder', events: await submit(api, api.tx.entityTransaction.shipOrder(orderId1, `tracking-${unique()}`), owner, 'shipOrder') });
artifact.steps.push({ step: 'updateTracking', events: await submit(api, api.tx.entityTransaction.updateTracking(orderId1, `tracking-updated-${unique()}`), owner, 'updateTracking') });
artifact.steps.push({ step: 'extendConfirmTimeout', events: await submit(api, api.tx.entityTransaction.extendConfirmTimeout(orderId1), actors.charlie, 'extendConfirmTimeout') });
artifact.steps.push({ step: 'confirmReceipt', events: await submit(api, api.tx.entityTransaction.confirmReceipt(orderId1), actors.charlie, 'confirmReceipt') });
artifact.order1 = toJson((await api.query.entityTransaction.orders(orderId1)).unwrap());

const orderId2 = Number((await api.query.entityTransaction.nextOrderId()).toString());
artifact.steps.push({ step: 'placeOrder2', events: await submit(api, api.tx.entityTransaction.placeOrder(productId, 1, `shipping-dave-${unique()}`, null, null, null, `note-dave-${unique()}`, null), actors.dave, 'placeOrder2') });
artifact.steps.push({ step: 'requestRefund', events: await submit(api, api.tx.entityTransaction.requestRefund(orderId2, `refund-${unique()}`), actors.dave, 'requestRefund') });
artifact.order2AfterRefundRequest = toJson((await api.query.entityTransaction.orders(orderId2)).unwrap());
artifact.steps.push({ step: 'approveRefund', events: await submit(api, api.tx.entityTransaction.approveRefund(orderId2), owner, 'approveRefund') });
artifact.order2AfterApprove = toJson((await api.query.entityTransaction.orders(orderId2)).unwrap());
artifact.steps.push({ step: 'unpublishProduct', events: await submit(api, api.tx.entityProduct.unpublishProduct(productId), owner, 'unpublishProduct') });
artifact.productAfterUnpublish = toJson((await api.query.entityProduct.products(productId)).unwrap());

artifact.passed = true;
await fs.writeFile(`${ARTIFACT_DIR}/entity-product-order-physical-flow.json`, JSON.stringify(safe(artifact), null, 2));
console.log(JSON.stringify({
  case: artifact.case,
  productId,
  orderId1,
  order1Status: artifact.order1.status,
  orderId2,
  order2Status: artifact.order2AfterApprove.status,
  productStatusAfterUnpublish: artifact.productAfterUnpublish.status,
}, null, 2));
await api.disconnect();
EOFJS
fi

if case_selected "commission-admin-controls"; then
  echo
  echo "==> commission-admin-controls"
  node --input-type=module <<'EOFJS'
import fs from 'node:fs/promises';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_URL = process.env.WS_URL;
const ARTIFACT_DIR = process.env.REMOTE_BF_ARTIFACT_DIR;

function toJson(v) { return v && typeof v.toJSON === 'function' ? v.toJSON() : v; }
function safe(v) { return JSON.parse(JSON.stringify(v, (_, cur) => typeof cur === 'bigint' ? cur.toString() : cur)); }
function decodeError(api, dispatchError) {
  if (!dispatchError) return null;
  if (dispatchError.isModule) {
    const meta = api.registry.findMetaError(dispatchError.asModule);
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
  }
  return dispatchError.toString();
}
async function submit(api, tx, signer, label) {
  return await new Promise((resolve, reject) => {
    let unsub = () => {};
    const timer = setTimeout(() => reject(new Error(`timeout ${label}`)), 240000);
    tx.signAndSend(signer, (result) => {
      if (!result.status.isFinalized) return;
      clearTimeout(timer);
      try { unsub(); } catch {}
      const err = decodeError(api, result.dispatchError);
      if (err) return reject(new Error(`${label}: ${err}`));
      resolve(result.events.map((r) => ({
        name: `${r.event.section}.${r.event.method}`,
        data: toJson(r.event.data),
      })));
    }).then((u) => { unsub = u; }).catch((e) => {
      clearTimeout(timer);
      reject(e);
    });
  });
}

const base = JSON.parse(await fs.readFile(`${ARTIFACT_DIR}/base-context.json`, 'utf8'));
await cryptoWaitReady();
const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
const keyring = new Keyring({ type: 'sr25519', ss58Format: 273 });
const owner = keyring.addFromUri(`//${base.ownerName[0].toUpperCase()}${base.ownerName.slice(1)}`);

const artifact = {
  case: 'commission-admin-controls',
  passed: false,
  entityId: base.entityId,
  steps: [],
};

artifact.steps.push({ step: 'setSingleLineConfig', events: await submit(api, api.tx.commissionSingleLine.setSingleLineConfig(base.entityId, 100, 100, 3, 3, 0, 4, 4), owner, 'setSingleLineConfig') });
artifact.steps.push({ step: 'updateSingleLineParams', events: await submit(api, api.tx.commissionSingleLine.updateSingleLineParams(base.entityId, 120, 150, null, null, null, null, null), owner, 'updateSingleLineParams') });
artifact.steps.push({ step: 'pauseSingleLine', events: await submit(api, api.tx.commissionSingleLine.pauseSingleLine(base.entityId), owner, 'pauseSingleLine') });
artifact.steps.push({ step: 'resumeSingleLine', events: await submit(api, api.tx.commissionSingleLine.resumeSingleLine(base.entityId), owner, 'resumeSingleLine') });
artifact.steps.push({ step: 'scheduleSingleLineChange', events: await submit(api, api.tx.commissionSingleLine.scheduleConfigChange(base.entityId, 130, 140, 3, 3, 0, 5, 5), owner, 'scheduleSingleLineChange') });
artifact.singleLine = {
  config: toJson(await api.query.commissionSingleLine.singleLineConfigs(base.entityId)),
  enabled: toJson(await api.query.commissionSingleLine.singleLineEnabled(base.entityId)),
  pending: toJson(await api.query.commissionSingleLine.pendingConfigChanges(base.entityId)),
};

artifact.steps.push({ step: 'setMultiLevelConfig', events: await submit(api, api.tx.commissionMultiLevel.setMultiLevelConfig(base.entityId, [{ rate: 200, required_directs: 0, required_team_size: 0, required_spent: 0 }, { rate: 100, required_directs: 0, required_team_size: 0, required_spent: 0 }], 300), owner, 'setMultiLevelConfig') });
artifact.steps.push({ step: 'addTier', events: await submit(api, api.tx.commissionMultiLevel.addTier(base.entityId, 2, { rate: 50, required_directs: 0, required_team_size: 0, required_spent: 0 }), owner, 'addTier') });
artifact.steps.push({ step: 'removeTier', events: await submit(api, api.tx.commissionMultiLevel.removeTier(base.entityId, 2), owner, 'removeTier') });
artifact.steps.push({ step: 'pauseMultiLevel', events: await submit(api, api.tx.commissionMultiLevel.pauseMultiLevel(base.entityId), owner, 'pauseMultiLevel') });
artifact.steps.push({ step: 'resumeMultiLevel', events: await submit(api, api.tx.commissionMultiLevel.resumeMultiLevel(base.entityId), owner, 'resumeMultiLevel') });
artifact.steps.push({ step: 'scheduleMultiLevelChange', events: await submit(api, api.tx.commissionMultiLevel.scheduleConfigChange(base.entityId, [{ rate: 220, required_directs: 0, required_team_size: 0, required_spent: 0 }, { rate: 80, required_directs: 0, required_team_size: 0, required_spent: 0 }], 320), owner, 'scheduleMultiLevelChange') });
artifact.multiLevel = {
  config: toJson(await api.query.commissionMultiLevel.multiLevelConfigs(base.entityId)),
  pending: toJson(await api.query.commissionMultiLevel.pendingConfigs(base.entityId)),
};

artifact.steps.push({ step: 'setPoolRewardConfig', events: await submit(api, api.tx.commissionPoolReward.setPoolRewardConfig(base.entityId, [[0, 10000]], 14400), owner, 'setPoolRewardConfig') });
artifact.steps.push({ step: 'pausePoolReward', events: await submit(api, api.tx.commissionPoolReward.pausePoolReward(base.entityId), owner, 'pausePoolReward') });
artifact.steps.push({ step: 'resumePoolReward', events: await submit(api, api.tx.commissionPoolReward.resumePoolReward(base.entityId), owner, 'resumePoolReward') });
artifact.steps.push({ step: 'schedulePoolRewardConfigChange', events: await submit(api, api.tx.commissionPoolReward.schedulePoolRewardConfigChange(base.entityId, [[0, 9000], [1, 1000]], 14400), owner, 'schedulePoolRewardConfigChange') });
artifact.poolReward = {
  config: toJson(await api.query.commissionPoolReward.poolRewardConfigs(base.entityId)),
  paused: toJson(await api.query.commissionPoolReward.poolRewardPaused(base.entityId)),
  pending: toJson(await api.query.commissionPoolReward.pendingPoolRewardConfig(base.entityId)),
};

artifact.passed = true;
await fs.writeFile(`${ARTIFACT_DIR}/commission-admin-controls.json`, JSON.stringify(safe(artifact), null, 2));
console.log(JSON.stringify({
  case: artifact.case,
  entityId: artifact.entityId,
  singleLinePending: artifact.singleLine.pending,
  multiLevelPending: artifact.multiLevel.pending,
  poolRewardPending: artifact.poolReward.pending,
}, null, 2));
await api.disconnect();
EOFJS
fi

if case_selected "nex-market-trade-flow"; then
  echo
  echo "==> nex-market-trade-flow"
  node --input-type=module <<'EOFJS'
import fs from 'node:fs/promises';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_URL = process.env.WS_URL;
const ARTIFACT_DIR = process.env.REMOTE_BF_ARTIFACT_DIR;
const NEX = 1_000_000_000_000n;
const SELLER_TRON = 'TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t';
const BUYER_TRON = 'TQn9Y2khEsLJW1ChVWFMSMeRDow5KcbLSE';

function toJson(v) { return v && typeof v.toJSON === 'function' ? v.toJSON() : v; }
function safe(v) { return JSON.parse(JSON.stringify(v, (_, cur) => typeof cur === 'bigint' ? cur.toString() : cur)); }
function decodeError(api, dispatchError) {
  if (!dispatchError) return null;
  if (dispatchError.isModule) {
    const meta = api.registry.findMetaError(dispatchError.asModule);
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`;
  }
  return dispatchError.toString();
}
async function submit(api, tx, signer, label) {
  return await new Promise((resolve, reject) => {
    let unsub = () => {};
    const timer = setTimeout(() => reject(new Error(`timeout ${label}`)), 240000);
    tx.signAndSend(signer, (result) => {
      if (!result.status.isFinalized) return;
      clearTimeout(timer);
      try { unsub(); } catch {}
      const err = decodeError(api, result.dispatchError);
      if (err) return reject(new Error(`${label}: ${err}`));
      resolve(result.events.map((r) => ({
        name: `${r.event.section}.${r.event.method}`,
        data: toJson(r.event.data),
      })));
    }).then((u) => { unsub = u; }).catch((e) => {
      clearTimeout(timer);
      reject(e);
    });
  });
}
async function free(api, address) {
  return BigInt((await api.query.system.account(address)).data.free.toString());
}
async function ensureBalance(api, alice, target, minimum) {
  const current = await free(api, target.address);
  if (current >= minimum) return current;
  await submit(api, api.tx.balances.transferKeepAlive(target.address, (minimum - current).toString()), alice, `fund-${target.address}`);
  return free(api, target.address);
}

await cryptoWaitReady();
const api = await ApiPromise.create({ provider: new WsProvider(WS_URL) });
const keyring = new Keyring({ type: 'sr25519', ss58Format: 273 });
const alice = keyring.addFromUri('//Alice');
const bob = keyring.addFromUri('//Bob');
const charlie = keyring.addFromUri('//Charlie');

await ensureBalance(api, alice, charlie, 65000n * NEX);

const artifact = {
  case: 'nex-market-trade-flow',
  passed: false,
  steps: [],
  priceProtection: toJson(await api.query.nexMarket.priceProtectionStore()),
  marketPaused: toJson(await api.query.nexMarket.marketPausedStore()),
};

const marketPrice = Number(artifact.priceProtection.initialPrice ?? artifact.priceProtection.initial_price ?? 0);
const orderId = Number((await api.query.nexMarket.nextOrderId()).toString());
artifact.steps.push({ step: 'placeSellOrder', events: await submit(api, api.tx.nexMarket.placeSellOrder((10n * NEX).toString(), marketPrice, SELLER_TRON, null), bob, 'placeSellOrder') });
artifact.orderAfterPlace = toJson(await api.query.nexMarket.orders(orderId));

const tradeId = Number((await api.query.nexMarket.nextUsdtTradeId()).toString());
artifact.steps.push({ step: 'reserveSellOrder', events: await submit(api, api.tx.nexMarket.reserveSellOrder(orderId, null, BUYER_TRON), charlie, 'reserveSellOrder') });
artifact.tradeAfterReserve = toJson(await api.query.nexMarket.usdtTrades(tradeId));
artifact.steps.push({ step: 'confirmPayment', events: await submit(api, api.tx.nexMarket.confirmPayment(tradeId), charlie, 'confirmPayment') });
artifact.steps.push({ step: 'sellerConfirmReceived', events: await submit(api, api.tx.nexMarket.sellerConfirmReceived(tradeId), bob, 'sellerConfirmReceived') });
artifact.orderFinal = toJson(await api.query.nexMarket.orders(orderId));
artifact.tradeFinal = toJson(await api.query.nexMarket.usdtTrades(tradeId));

artifact.passed = true;
await fs.writeFile(`${ARTIFACT_DIR}/nex-market-trade-flow.json`, JSON.stringify(safe(artifact), null, 2));
console.log(JSON.stringify({
  case: artifact.case,
  orderId,
  orderStatusAfterPlace: artifact.orderAfterPlace.status,
  tradeId,
  tradeStatusAfterReserve: artifact.tradeAfterReserve.status,
  finalTradeStatus: artifact.tradeFinal.status,
  finalOrderStatus: artifact.orderFinal.status,
}, null, 2));
await api.disconnect();
EOFJS
fi

export REMOTE_FLOW_SELECTED_CASES="$(IFS=,; echo "${SELECTED_CASES[*]-}")"
python3 "$SCRIPT_DIR/build-report.py"

echo
echo "[remote-business-flows] done"
echo "[remote-business-flows] report=$SCRIPT_DIR/REPORT.md"
echo "[remote-business-flows] latest=$ARTIFACT_DIR/latest.json"
