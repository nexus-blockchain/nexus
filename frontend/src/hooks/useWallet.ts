"use client";

import { useState, useCallback, useEffect } from "react";
import { useWalletStore } from "@/stores/wallet";

interface InjectedAccount {
  address: string;
  name?: string;
}

interface WalletHook {
  accounts: InjectedAccount[];
  isConnecting: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  selectAccount: (address: string) => void;
}

export function useWallet(): WalletHook {
  const [accounts, setAccounts] = useState<InjectedAccount[]>([]);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { connect: storeConnect, disconnect: storeDisconnect } = useWalletStore();

  const connect = useCallback(async () => {
    setIsConnecting(true);
    setError(null);
    try {
      const { web3Enable, web3Accounts } = await import(
        "@polkadot/extension-dapp"
      );
      const extensions = await web3Enable("NEXUS Entity Manager");
      if (extensions.length === 0) {
        throw new Error("No wallet extension found. Please install Polkadot.js, Talisman, or SubWallet.");
      }
      const allAccounts = await web3Accounts();
      const mapped = allAccounts.map((a) => ({
        address: a.address,
        name: a.meta.name,
      }));
      setAccounts(mapped);
      if (mapped.length > 0) {
        storeConnect(mapped[0].address, mapped[0].name);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to connect wallet");
    } finally {
      setIsConnecting(false);
    }
  }, [storeConnect]);

  const disconnect = useCallback(() => {
    storeDisconnect();
    setAccounts([]);
  }, [storeDisconnect]);

  const selectAccount = useCallback(
    (address: string) => {
      const account = accounts.find((a) => a.address === address);
      if (account) {
        storeConnect(account.address, account.name);
      }
    },
    [accounts, storeConnect]
  );

  return { accounts, isConnecting, error, connect, disconnect, selectAccount };
}
