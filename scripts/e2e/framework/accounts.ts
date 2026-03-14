import type { ApiPromise } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { submitTx } from './api.js';
import { assertTxSuccess } from './assert.js';
import { DevActors } from './types.js';
import { nex } from './units.js';
import { NEXUS_SS58_FORMAT } from '../../utils/ss58.js';

let keyring: Keyring | undefined;
let cryptoReadyPromise: Promise<boolean> | undefined;

function getKeyring(): Keyring {
  keyring ??= new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });
  return keyring;
}

async function ensureCryptoReady(): Promise<void> {
  cryptoReadyPromise ??= cryptoWaitReady();
  await cryptoReadyPromise;
}

export async function getDevActors(): Promise<DevActors> {
  await ensureCryptoReady();
  const keyring = getKeyring();
  return {
    alice: keyring.addFromUri('//Alice'),
    bob: keyring.addFromUri('//Bob'),
    charlie: keyring.addFromUri('//Charlie'),
    dave: keyring.addFromUri('//Dave'),
    eve: keyring.addFromUri('//Eve'),
    ferdie: keyring.addFromUri('//Ferdie'),
  };
}

export async function readFreeBalance(api: ApiPromise, address: string): Promise<bigint> {
  const account = await api.query.system.account(address);
  return BigInt(((account as any).data.free as any).toString());
}

export async function ensureActorBalance(api: ApiPromise, actors: DevActors, minNex: number): Promise<void> {
  await ensureNamedActorBalance(api, actors, Object.keys(actors), minNex);
}

export async function ensureNamedActorBalance(
  api: ApiPromise,
  actors: DevActors,
  actorNames: string[],
  minNex: number,
): Promise<void> {
  const minimum = nex(minNex);
  const faucet = actors.alice;

  for (const name of actorNames) {
    const actor = actors[name];
    if (!actor) {
      continue;
    }
    if (name === 'alice') {
      continue;
    }

    const free = await readFreeBalance(api, actor.address);
    if (free >= minimum) {
      continue;
    }

    const delta = minimum - free;
    const tx = api.tx.balances.transferKeepAlive(actor.address, delta.toString());
    const receipt = await submitTx(api, tx, faucet, `fund ${name}`);
    assertTxSuccess(receipt, `fund ${name}`);
  }
}
