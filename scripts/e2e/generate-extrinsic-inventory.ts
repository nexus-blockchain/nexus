#!/usr/bin/env tsx
/**
 * 生成链端 extrinsic inventory，便于按接口追踪测试覆盖情况。
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

interface RuntimePalletMeta {
  alias: string;
  runtimeIndex: number;
  packageName: string;
}

interface ExtrinsicEntry {
  pallet: string;
  runtimeAlias: string | null;
  runtimeIndex: number | null;
  inRuntime: boolean;
  sourceFile: string;
  hasUnitTests: boolean;
  callIndex: number;
  extrinsic: string;
  camelName: string;
  planFiles: string[];
  flowFiles: string[];
  status: 'implemented' | 'planned' | 'unplanned' | 'not_in_runtime';
}

interface InventoryPayload {
  generatedAt: string;
  repoRoot: string;
  summary: {
    runtimePallets: number;
    localCallablePallets: number;
    extrinsics: number;
    withUnitTests: number;
    byStatus: Record<string, number>;
  };
  pallets: Array<{
    pallet: string;
    runtimeAlias: string | null;
    runtimeIndex: number | null;
    inRuntime: boolean;
    sourceFile: string;
    hasUnitTests: boolean;
    extrinsicCount: number;
  }>;
  extrinsics: ExtrinsicEntry[];
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..', '..');
const outputPath = path.resolve(repoRoot, 'scripts/docs/extrinsic_inventory.json');
const planFiles = [
  path.resolve(repoRoot, 'scripts/docs/NEXUS_TEST_PLAN.md'),
  path.resolve(repoRoot, 'scripts/docs/NEXUS_TEST_PLAN_PART2.md'),
  path.resolve(repoRoot, 'scripts/docs/NEXUS_TEST_PLAN_PART3.md'),
];
const flowDir = path.resolve(repoRoot, 'scripts/e2e/flows');

function walk(dir: string, filter?: (filePath: string) => boolean): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...walk(fullPath, filter));
      continue;
    }
    if (!filter || filter(fullPath)) {
      files.push(fullPath);
    }
  }

  return files;
}

function readPackageName(cargoTomlPath: string): string | null {
  const content = fs.readFileSync(cargoTomlPath, 'utf-8');
  const match = content.match(/^name\s*=\s*"([^"]+)"/m);
  return match ? match[1] : null;
}

function snakeToCamel(input: string): string {
  return input.replace(/_([a-z])/g, (_, char: string) => char.toUpperCase());
}

function parseRuntimePallets(runtimeLibPath: string): Map<string, RuntimePalletMeta> {
  const lines = fs.readFileSync(runtimeLibPath, 'utf-8').split('\n');
  const pallets = new Map<string, RuntimePalletMeta>();
  let pendingIndex: number | null = null;

  for (const line of lines) {
    const indexMatch = line.match(/#\[runtime::pallet_index\((\d+)\)\]/);
    if (indexMatch) {
      pendingIndex = Number(indexMatch[1]);
      continue;
    }

    const aliasMatch = line.match(/pub type (\w+) = (pallet_[\w]+)(?:<[^>]+>)?;/);
    if (!aliasMatch || pendingIndex === null) {
      continue;
    }

    const alias = aliasMatch[1];
    const rustPallet = aliasMatch[2];
    const packageName = rustPallet.replace(/^pallet_/, 'pallet-').replaceAll('_', '-');
    pallets.set(packageName, {
      alias,
      runtimeIndex: pendingIndex,
      packageName,
    });
    pendingIndex = null;
  }

  return pallets;
}

function collectPlanContents(): Array<{ fileName: string; content: string }> {
  return planFiles
    .filter(filePath => fs.existsSync(filePath))
    .map(filePath => ({ fileName: path.basename(filePath), content: fs.readFileSync(filePath, 'utf-8') }));
}

function collectFlowContents(): Array<{ fileName: string; content: string }> {
  return walk(flowDir, filePath => filePath.endsWith('.ts'))
    .map(filePath => ({
      fileName: path.relative(repoRoot, filePath),
      content: fs.readFileSync(filePath, 'utf-8'),
    }));
}

function collectExtrinsics(): InventoryPayload {
  const runtimePallets = parseRuntimePallets(path.resolve(repoRoot, 'runtime/src/lib.rs'));
  const planContents = collectPlanContents();
  const flowContents = collectFlowContents();
  const libFiles = walk(path.resolve(repoRoot, 'pallets'), filePath => filePath.endsWith('src/lib.rs'));

  const extrinsics: ExtrinsicEntry[] = [];
  const palletSummaries = new Map<string, InventoryPayload['pallets'][number]>();

  for (const libFile of libFiles) {
    const content = fs.readFileSync(libFile, 'utf-8');
    if (!content.includes('#[pallet::call]')) {
      continue;
    }

    const cargoTomlPath = path.resolve(path.dirname(path.dirname(libFile)), 'Cargo.toml');
    if (!fs.existsSync(cargoTomlPath)) {
      continue;
    }

    const pallet = readPackageName(cargoTomlPath);
    if (!pallet) {
      continue;
    }

    const runtimeMeta = runtimePallets.get(pallet) ?? null;
    const hasUnitTests = fs.existsSync(path.resolve(path.dirname(libFile), 'tests.rs'));
    const lines = content.split('\n');

    if (!palletSummaries.has(pallet)) {
      palletSummaries.set(pallet, {
        pallet,
        runtimeAlias: runtimeMeta?.alias ?? null,
        runtimeIndex: runtimeMeta?.runtimeIndex ?? null,
        inRuntime: !!runtimeMeta,
        sourceFile: path.relative(repoRoot, libFile),
        hasUnitTests,
        extrinsicCount: 0,
      });
    }

    for (let i = 0; i < lines.length; i++) {
      const callIndexMatch = lines[i].match(/#\[pallet::call_index\((\d+)\)\]/);
      if (!callIndexMatch) {
        continue;
      }

      let extrinsicName: string | null = null;
      for (let j = i + 1; j < Math.min(i + 40, lines.length); j++) {
        const fnMatch = lines[j].match(/pub fn (\w+)/);
        if (fnMatch) {
          extrinsicName = fnMatch[1];
          break;
        }
      }
      if (!extrinsicName) {
        continue;
      }

      const camelName = snakeToCamel(extrinsicName);
      const matchingPlanFiles = planContents
        .filter(file => file.content.includes(extrinsicName))
        .map(file => file.fileName);
      const matchingFlowFiles = flowContents
        .filter(file => file.content.includes(extrinsicName) || file.content.includes(camelName))
        .map(file => file.fileName);

      let status: ExtrinsicEntry['status'];
      if (!runtimeMeta) {
        status = 'not_in_runtime';
      } else if (matchingFlowFiles.length > 0) {
        status = 'implemented';
      } else if (matchingPlanFiles.length > 0) {
        status = 'planned';
      } else {
        status = 'unplanned';
      }

      extrinsics.push({
        pallet,
        runtimeAlias: runtimeMeta?.alias ?? null,
        runtimeIndex: runtimeMeta?.runtimeIndex ?? null,
        inRuntime: !!runtimeMeta,
        sourceFile: path.relative(repoRoot, libFile),
        hasUnitTests,
        callIndex: Number(callIndexMatch[1]),
        extrinsic: extrinsicName,
        camelName,
        planFiles: matchingPlanFiles,
        flowFiles: matchingFlowFiles,
        status,
      });

      const summary = palletSummaries.get(pallet)!;
      summary.extrinsicCount++;
    }
  }

  extrinsics.sort((a, b) => {
    if (a.pallet !== b.pallet) return a.pallet.localeCompare(b.pallet);
    return a.callIndex - b.callIndex;
  });

  const byStatus: Record<string, number> = {
    implemented: 0,
    planned: 0,
    unplanned: 0,
    not_in_runtime: 0,
  };
  for (const entry of extrinsics) {
    byStatus[entry.status] = (byStatus[entry.status] ?? 0) + 1;
  }

  const runtimeCallablePallets = [...palletSummaries.values()].filter(item => item.inRuntime).length;

  return {
    generatedAt: new Date().toISOString(),
    repoRoot,
    summary: {
      runtimePallets: runtimeCallablePallets,
      localCallablePallets: palletSummaries.size,
      extrinsics: extrinsics.length,
      withUnitTests: [...palletSummaries.values()].filter(item => item.hasUnitTests).length,
      byStatus,
    },
    pallets: [...palletSummaries.values()].sort((a, b) => a.pallet.localeCompare(b.pallet)),
    extrinsics,
  };
}

function main(): void {
  const payload = collectExtrinsics();
  fs.writeFileSync(outputPath, JSON.stringify(payload, null, 2) + '\n', 'utf-8');

  console.log(`Wrote ${payload.extrinsics.length} extrinsics to ${path.relative(repoRoot, outputPath)}`);
  console.log(`Runtime pallets: ${payload.summary.runtimePallets}`);
  console.log(`Local callable pallets: ${payload.summary.localCallablePallets}`);
  console.log(`Status counts: ${JSON.stringify(payload.summary.byStatus)}`);
}

main();
