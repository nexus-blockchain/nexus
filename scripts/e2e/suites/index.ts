import { TestSuite } from '../framework/types.js';
import { chainHealthSuite } from './chain-health.js';
import { entityLifecycleSuite } from './entity-lifecycle.js';
import { nexMarketSmokeSuite } from './nex-market-smoke.js';
import { runtimeContractsSuite } from './runtime-contracts.js';

export const ALL_SUITES: TestSuite[] = [
  chainHealthSuite,
  runtimeContractsSuite,
  entityLifecycleSuite,
  nexMarketSmokeSuite,
];

export const SUITE_MAP = new Map(ALL_SUITES.map((suite) => [suite.id, suite]));
