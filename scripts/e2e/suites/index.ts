import { TestSuite } from '../framework/types.js';
import { chainHealthSuite } from './chain-health.js';
import { entityCommerceCommissionFlowSuite } from './entity-commerce-commission-flow.js';
import { entityLifecycleSuite } from './entity-lifecycle.js';
import { nexMarketSmokeSuite } from './nex-market-smoke.js';
import { phase1BuyOrderAcceptanceFlowSuite } from './phase1-buy-order-acceptance-flow.js';
import { phase1ShopClosingGuardCancelSuite } from './phase1-shop-closing-guard-cancel.js';
import { phase1ServiceOrderLifecycleSuite } from './phase1-service-order-lifecycle.js';
import { phase1SpendDrivenLevelUpgradeSuite } from './phase1-spend-driven-level-upgrade.js';
import { phase1ThirdPartyPaymentOrderSuite } from './phase1-third-party-payment-order.js';
import { phase1TokenPaymentTokenCommissionSuite } from './phase1-token-payment-token-commission.js';
import { remoteBusinessFlowsSuite } from './remote-business-flows.js';
import { runtimeContractsSuite } from './runtime-contracts.js';

export const DEFAULT_SUITES: TestSuite[] = [
  chainHealthSuite,
  runtimeContractsSuite,
  entityLifecycleSuite,
  entityCommerceCommissionFlowSuite,
  nexMarketSmokeSuite,
  remoteBusinessFlowsSuite,
];

// Phase 1 registration is opt-in for default execution. These suites become
// addressable via `--suite` and `e2e:list`, but do not run on bare `npm run e2e`.
// Exact S1-06 finalize coverage is still runtime-blocked by the current 7-day
// grace period; the registered substitute only covers close + guards + cancel.
export const PHASE1_SUITES: TestSuite[] = [
  phase1ServiceOrderLifecycleSuite,
  phase1ThirdPartyPaymentOrderSuite,
  phase1TokenPaymentTokenCommissionSuite,
  phase1BuyOrderAcceptanceFlowSuite,
  phase1SpendDrivenLevelUpgradeSuite,
  phase1ShopClosingGuardCancelSuite,
];

export const ALL_SUITES: TestSuite[] = [
  ...DEFAULT_SUITES,
  ...PHASE1_SUITES,
];

export const SUITE_MAP = new Map(ALL_SUITES.map((suite) => [suite.id, suite]));
