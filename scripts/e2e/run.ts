#!/usr/bin/env tsx

import { connectApi, disconnectApi } from './framework/api.js';
import { runSuites } from './framework/runner.js';
import { TestSuite } from './framework/types.js';
import { ALL_SUITES, DEFAULT_SUITES, SUITE_MAP } from './suites/index.js';

interface CliSelection {
  listOnly: boolean;
  suites: TestSuite[];
  label: string;
}

function parseArgs(argv: string[]): CliSelection {
  if (argv.includes('--list')) {
    return { listOnly: true, suites: ALL_SUITES, label: 'list' };
  }

  const suiteIndex = argv.indexOf('--suite');
  if (suiteIndex === -1) {
    return { listOnly: false, suites: DEFAULT_SUITES, label: 'default' };
  }

  const requested = argv.slice(suiteIndex + 1).filter((arg) => !arg.startsWith('--'));
  if (requested.length === 0) {
    throw new Error(`Missing suite ids after --suite. Available: ${ALL_SUITES.map((suite) => suite.id).join(', ')}`);
  }

  const suites = requested.map((id) => {
    const suite = SUITE_MAP.get(id);
    if (!suite) {
      throw new Error(`Unknown suite: ${id}. Available: ${ALL_SUITES.map((item) => item.id).join(', ')}`);
    }
    return suite;
  });

  return { listOnly: false, suites, label: requested.join(', ') };
}

async function main(): Promise<void> {
  const selection = parseArgs(process.argv.slice(2));
  const traceBootstrap = process.env.E2E_TRACE_BOOTSTRAP === '1';

  if (selection.listOnly) {
    for (const suite of ALL_SUITES) {
      console.log(`${suite.id.padEnd(20)} ${suite.title} — ${suite.description}`);
    }
    return;
  }

  console.log(`Running suites: ${selection.label}`);

  if (traceBootstrap) {
    console.log('[bootstrap] connecting api');
  }
  const api = await connectApi();
  try {
    if (traceBootstrap) {
      console.log('[bootstrap] api connected');
      console.log('[bootstrap] loading actors');
    }
    const { getDevActors } = await import('./framework/accounts.js');
    const actors = await getDevActors();
    if (traceBootstrap) {
      console.log('[bootstrap] actors ready');
    }
    const allPassed = await runSuites(api, actors, selection.suites);
    process.exitCode = allPassed ? 0 : 1;
  } finally {
    await disconnectApi(api);
  }
}

main().catch((error) => {
  console.error(`E2E runner failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
