import { describe, it, expect, beforeEach } from 'vitest';
import { useWalletStore } from './wallet-store';

describe('useWalletStore', () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useWalletStore.setState({
      address: null,
      name: null,
      source: null,
      isConnected: false,
      balance: BigInt(0),
    });
  });

  it('should have correct initial state', () => {
    const state = useWalletStore.getState();
    expect(state.address).toBeNull();
    expect(state.name).toBeNull();
    expect(state.source).toBeNull();
    expect(state.isConnected).toBe(false);
    expect(state.balance).toBe(BigInt(0));
  });

  it('should set wallet info and mark as connected', () => {
    const { setWallet } = useWalletStore.getState();
    setWallet('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY', 'Alice', 'polkadot-js');

    const state = useWalletStore.getState();
    expect(state.address).toBe('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY');
    expect(state.name).toBe('Alice');
    expect(state.source).toBe('polkadot-js');
    expect(state.isConnected).toBe(true);
  });

  it('should update balance', () => {
    const { setBalance } = useWalletStore.getState();
    setBalance(BigInt('1000000000000'));

    expect(useWalletStore.getState().balance).toBe(BigInt('1000000000000'));
  });

  it('should disconnect and reset all state', () => {
    const { setWallet, setBalance, disconnect } = useWalletStore.getState();

    // First connect
    setWallet('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY', 'Alice', 'talisman');
    setBalance(BigInt('5000000000000'));

    // Then disconnect
    disconnect();

    const state = useWalletStore.getState();
    expect(state.address).toBeNull();
    expect(state.name).toBeNull();
    expect(state.source).toBeNull();
    expect(state.isConnected).toBe(false);
    expect(state.balance).toBe(BigInt(0));
  });

  it('should allow switching wallets by calling setWallet again', () => {
    const { setWallet } = useWalletStore.getState();

    setWallet('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY', 'Alice', 'polkadot-js');
    setWallet('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty', 'Bob', 'subwallet-js');

    const state = useWalletStore.getState();
    expect(state.address).toBe('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty');
    expect(state.name).toBe('Bob');
    expect(state.source).toBe('subwallet-js');
    expect(state.isConnected).toBe(true);
  });
});
