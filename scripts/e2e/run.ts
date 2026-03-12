#!/usr/bin/env tsx

import { getDevActors } from './framework/accounts.js';
import { connectApi, disconnectApi } from './framework/api.js';
import { runSuites } from './framework/runner.js';
import { TestSuite } from './framework/types.js';
import { ALL_SUITES, SUITE_MAP } from './suites/index.js';

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
    return { listOnly: false, suites: ALL_SUITES, label: 'all' };
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

  if (selection.listOnly) {
    for (const suite of ALL_SUITES) {
      console.log(`${suite.id.padEnd(20)} ${suite.title} — ${suite.description}`);
    }
    return;
  }

  console.log(`Running suites: ${selection.label}`);

  const api = await connectApi();
  try {
    const allPassed = await runSuites(api, getDevActors(), selection.suites);
    process.exitCode = allPassed ? 0 : 1;
  } finally {
    await disconnectApi(api);
  }
}

main().catch((error) => {
  console.error(`E2E runner failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
