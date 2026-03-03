import { create } from "zustand";

interface WalletState {
  address: string | null;
  name: string | null;
  isConnected: boolean;
  balance: bigint;
  connect: (address: string, name?: string) => void;
  disconnect: () => void;
  setBalance: (balance: bigint) => void;
}

export const useWalletStore = create<WalletState>((set) => ({
  address: null,
  name: null,
  isConnected: false,
  balance: BigInt(0),
  connect: (address, name) =>
    set({ address, name: name || null, isConnected: true }),
  disconnect: () =>
    set({ address: null, name: null, isConnected: false, balance: BigInt(0) }),
  setBalance: (balance) => set({ balance }),
}));
