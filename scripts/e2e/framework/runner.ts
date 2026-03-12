import { ApiPromise } from '@polkadot/api';
import { ensureFundedActors, readPreferredMarketPrice } from './bootstrap.js';
import { captureChainSnapshot } from './api.js';
import { DevActors, SuiteContext, TestSuite } from './types.js';

function asErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export async function runSuites(api: ApiPromise, actors: DevActors, suites: TestSuite[]): Promise<boolean> {
  const chain = await captureChainSnapshot(api);
  let allPassed = true;
  let passedSuites = 0;

  console.log(`Chain: ${chain.chain} (${chain.specName} v${chain.specVersion})`);

  for (const suite of suites) {
    let suitePassed = true;
    let stepCount = 0;

    console.log(`\n• ${suite.title} [${suite.id}]`);
    console.log(`  ${suite.description}`);

    const ctx: SuiteContext = {
      api,
      actors,
      chain,
      step: async (name, fn) => {
        const startedAt = Date.now();
        stepCount += 1;

        try {
          const result = await fn();
          console.log(`  ✓ ${name} (${Date.now() - startedAt}ms)`);
          return result;
        } catch (error) {
          suitePassed = false;
          console.log(`  ✗ ${name} (${Date.now() - startedAt}ms) — ${asErrorMessage(error)}`);
          throw error;
        }
      },
      note: (message) => console.log(`  - ${message}`),
      ensureFunds: (minNex: number = 25_000) => ensureFundedActors(api, actors, minNex),
      readMarketPrice: () => readPreferredMarketPrice(api),
    };

    try {
      await suite.run(ctx);
      if (suitePassed) {
        passedSuites += 1;
        console.log(`  ✓ suite passed (${stepCount} step${stepCount === 1 ? '' : 's'})`);
      } else {
        allPassed = false;
      }
    } catch (error) {
      allPassed = false;
      console.log(`  ! suite failed — ${asErrorMessage(error)}`);
    }
  }

  const failedSuites = suites.length - passedSuites;
  console.log(`\nCompleted ${suites.length} suite(s): ${passedSuites} passed, ${failedSuites} failed`);
  return allPassed;
}
