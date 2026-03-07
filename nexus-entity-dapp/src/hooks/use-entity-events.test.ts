import { describe, it, expect } from 'vitest';
import {
  generateEventSummary,
  filterEntityEvent,
  ENTITY_EVENT_CONFIGS,
  type EntityEventConfig,
} from './use-entity-events';

// ─── Helper: create a mock EventRecord ──────────────────────

function mockEventRecord(section: string, method: string, entityId: number): any {
  return {
    event: {
      section,
      method,
      data: [
        {
          toString: () => String(entityId),
        },
      ],
    },
  };
}

// ─── generateEventSummary ───────────────────────────────────

describe('generateEventSummary', () => {
  it('returns a known summary for EntityRegistry.EntitySuspendedLowFund', () => {
    const summary = generateEventSummary('entityRegistry', 'EntitySuspendedLowFund');
    expect(summary).toBe('Entity suspended due to low funds');
  });

  it('returns a known summary for entityOrder.OrderPlaced', () => {
    const summary = generateEventSummary('entityOrder', 'OrderPlaced');
    expect(summary).toBe('A new order has been placed');
  });

  it('returns a known summary for entityMarket.CircuitBreakerTriggered', () => {
    const summary = generateEventSummary('entityMarket', 'CircuitBreakerTriggered');
    expect(summary).toBe('Market circuit breaker has been triggered');
  });

  it('returns fallback for unknown pallet/event', () => {
    const summary = generateEventSummary('unknownPallet', 'UnknownEvent');
    expect(summary).toBe('unknownPallet.UnknownEvent');
  });

  it('returns non-empty string for all configured events', () => {
    for (const config of ENTITY_EVENT_CONFIGS) {
      for (const eventName of config.events) {
        const summary = generateEventSummary(config.pallet, eventName);
        expect(summary.length).toBeGreaterThan(0);
      }
    }
  });
});

// ─── filterEntityEvent ──────────────────────────────────────

describe('filterEntityEvent', () => {
  it('matches an event with correct pallet, method, and entityId', () => {
    const record = mockEventRecord('entityShop', 'ShopCreated', 42);
    const result = filterEntityEvent(record, 42);
    expect(result).not.toBeNull();
    expect(result!.eventName).toBe('ShopCreated');
    expect(result!.config.pallet).toBe('entityShop');
  });

  it('returns null when entityId does not match', () => {
    const record = mockEventRecord('entityShop', 'ShopCreated', 99);
    const result = filterEntityEvent(record, 42);
    expect(result).toBeNull();
  });

  it('returns null for unknown pallet', () => {
    const record = mockEventRecord('unknownPallet', 'SomeEvent', 42);
    const result = filterEntityEvent(record, 42);
    expect(result).toBeNull();
  });

  it('returns null for known pallet but unknown event', () => {
    const record = mockEventRecord('entityShop', 'UnknownEvent', 42);
    const result = filterEntityEvent(record, 42);
    expect(result).toBeNull();
  });

  it('matches events from all 8 configured pallets', () => {
    const entityId = 7;
    const matchedPallets = new Set<string>();

    for (const config of ENTITY_EVENT_CONFIGS) {
      const record = mockEventRecord(config.pallet, config.events[0], entityId);
      const result = filterEntityEvent(record, entityId);
      if (result) matchedPallets.add(result.config.pallet);
    }

    expect(matchedPallets.size).toBe(8);
  });
});

// ─── ENTITY_EVENT_CONFIGS ───────────────────────────────────

describe('ENTITY_EVENT_CONFIGS', () => {
  it('covers all 8 required pallets', () => {
    const pallets = ENTITY_EVENT_CONFIGS.map((c) => c.pallet);
    expect(pallets).toContain('entityRegistry');
    expect(pallets).toContain('entityShop');
    expect(pallets).toContain('entityToken');
    expect(pallets).toContain('entityOrder');
    expect(pallets).toContain('entityMarket');
    expect(pallets).toContain('entityMember');
    expect(pallets).toContain('entityCommission');
    expect(pallets).toContain('entityGovernance');
  });

  it('each config has at least one event', () => {
    for (const config of ENTITY_EVENT_CONFIGS) {
      expect(config.events.length).toBeGreaterThan(0);
    }
  });

  it('each config has a filter function', () => {
    for (const config of ENTITY_EVENT_CONFIGS) {
      expect(typeof config.filter).toBe('function');
    }
  });

  it('each config returns non-empty invalidateKeys', () => {
    for (const config of ENTITY_EVENT_CONFIGS) {
      const keys = config.invalidateKeys(1);
      expect(keys.length).toBeGreaterThan(0);
      for (const key of keys) {
        expect(key.length).toBeGreaterThan(0);
      }
    }
  });

  it('invalidateKeys always include the entityId', () => {
    const entityId = 123;
    for (const config of ENTITY_EVENT_CONFIGS) {
      const keys = config.invalidateKeys(entityId);
      for (const key of keys) {
        expect(key).toContain(entityId);
      }
    }
  });
});
