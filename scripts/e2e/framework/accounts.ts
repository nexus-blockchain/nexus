import { ApiPromise } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { submitTx } from './api.js';
import { assertTxSuccess } from './assert.js';
import { DevActors } from './types.js';
import { nex } from './units.js';
import { NEXUS_SS58_FORMAT } from '../../utils/ss58.js';

const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });

export function getDevActors(): DevActors {
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
  const minimum = nex(minNex);
  const faucet = actors.alice;

  for (const [name, actor] of Object.entries(actors)) {
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
