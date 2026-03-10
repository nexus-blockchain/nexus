#!/usr/bin/env tsx
/**
 * 快速覆盖率检查脚本
 */
import * as path from 'path';
import { fileURLToPath } from 'url';
import {
  parseTestPlan,
  applyCoverage,
  generateCoverageReport,
  printCoverageReport,
} from './core/coverage-tracker.js';
import { COVERAGE_MAP } from './core/coverage-map.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const planDir = path.resolve(__dirname, '../docs');

const cases = parseTestPlan(planDir);
console.log(`解析到 ${cases.length} 个测试用例`);

if (cases.length === 0) {
  console.log('未找到用例，检查 docs/ 目录');
  process.exit(1);
}

applyCoverage(cases, COVERAGE_MAP);
const report = generateCoverageReport(cases);
printCoverageReport(report);
