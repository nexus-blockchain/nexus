import { Keyring } from '@polkadot/keyring';
import { KeyringPair } from '@polkadot/keyring/types';
import { NEXUS_SS58_FORMAT } from './ss58.js';

const keyring = new Keyring({ type: 'sr25519', ss58Format: NEXUS_SS58_FORMAT });

export const ALICE_URI = '//Alice';
export const BOB_URI = '//Bob';
export const CHARLIE_URI = '//Charlie';
export const DAVE_URI = '//Dave';
export const EVE_URI = '//Eve';

export function getAlice(): KeyringPair {
  return keyring.addFromUri(ALICE_URI);
}

export function getBob(): KeyringPair {
  return keyring.addFromUri(BOB_URI);
}

export function getCharlie(): KeyringPair {
  return keyring.addFromUri(CHARLIE_URI);
}

export function getDave(): KeyringPair {
  return keyring.addFromUri(DAVE_URI);
}

export function getEve(): KeyringPair {
  return keyring.addFromUri(EVE_URI);
}

export function getAccountFromUri(uri: string): KeyringPair {
  return keyring.addFromUri(uri);
}

export function formatAddress(address: string): string {
  if (address.length <= 16) return address;
  return `${address.slice(0, 8)}...${address.slice(-8)}`;
}

export function logAccount(name: string, account: KeyringPair): void {
  console.log(`👤 ${name}: ${formatAddress(account.address)}`);
}
