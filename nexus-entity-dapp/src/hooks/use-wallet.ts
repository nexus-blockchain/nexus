'use client';

import { useCallback } from 'react';
import { web3Accounts, web3Enable, web3FromSource } from '@polkadot/extension-dapp';
import type { InjectedAccountWithMeta } from '@polkadot/extension-inject/types';
import { useWalletStore } from '@/stores/wallet-store';

const APP_NAME = 'NEXUS Entity dApp';

// Supported wallet extensions
const SUPPORTED_WALLETS = ['polkadot-js', 'talisman', 'subwallet-js'] as const;
export type SupportedWallet = (typeof SUPPORTED_WALLETS)[number];

export function useWallet() {
  const { address, name, source, isConnected, balance, setWallet, setBalance, disconnect } =
    useWalletStore();

  /** Enable wallet extensions and get available accounts */
  const getAccounts = useCallback(async (): Promise<InjectedAccountWithMeta[]> => {
    const extensions = await web3Enable(APP_NAME);
    if (extensions.length === 0) {
      throw new Error('No wallet extension found. Please install Polkadot.js, Talisman, or SubWallet.');
    }
    return web3Accounts();
  }, []);

  /** Connect to a specific account */
  const connect = useCallback(
    async (account: InjectedAccountWithMeta) => {
      const injector = await web3FromSource(account.meta.source);
      if (!injector) {
        throw new Error(`Failed to connect to ${account.meta.source}`);
      }
      setWallet(
        account.address,
        account.meta.name || 'Unknown',
        account.meta.source,
      );
    },
    [setWallet],
  );

  /** Get signer for transaction signing */
  const getSigner = useCallback(async () => {
    if (!source) throw new Error('Wallet not connected');
    const injector = await web3FromSource(source);
    return injector.signer;
  }, [source]);

  return {
    address,
    name,
    source,
    isConnected,
    balance,
    setBalance,
    getAccounts,
    connect,
    disconnect,
    getSigner,
    supportedWallets: SUPPORTED_WALLETS,
  };
}
