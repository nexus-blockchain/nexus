import { create } from 'zustand';

interface WalletState {
  address: string | null;
  name: string | null;
  source: string | null;  // wallet extension name
  isConnected: boolean;
  balance: bigint;

  // Actions
  setWallet: (address: string, name: string, source: string) => void;
  setBalance: (balance: bigint) => void;
  disconnect: () => void;
}

export const useWalletStore = create<WalletState>((set) => ({
  address: null,
  name: null,
  source: null,
  isConnected: false,
  balance: BigInt(0),

  setWallet: (address, name, source) =>
    set({ address, name, source, isConnected: true }),

  setBalance: (balance) => set({ balance }),

  disconnect: () =>
    set({ address: null, name: null, source: null, isConnected: false, balance: BigInt(0) }),
}));
