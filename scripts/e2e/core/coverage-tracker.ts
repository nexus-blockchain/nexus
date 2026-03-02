/**
 * 测试计划覆盖率追踪器
 *
 * 解析 NEXUS_TEST_PLAN*.md，映射到已实现的 Flow，计算覆盖率
 */

import * as fs from 'fs';
import * as path from 'path';

export interface TestCase {
  id: string;        // e.g. "ER-001"
  module: string;    // e.g. "Entity Registry"
  description: string;
  role: string;
  type: string;      // 正向/负向/流程/权限/安全/超时/边界/功能/集成
  priority: string;  // P0/P1/P2
  covered: boolean;
  coveredBy?: string; // Flow name
}

export interface CoverageReport {
  totalCases: number;
  coveredCases: number;
  coveragePercent: number;
  byModule: Record<string, { total: number; covered: number; percent: number }>;
  byPriority: Record<string, { total: number; covered: number; percent: number }>;
  uncoveredP0: TestCase[];
  uncoveredP1: TestCase[];
}

/** 已实现的 Flow 覆盖映射: Flow name → 覆盖的测试用例 ID 列表 */
export type CoverageMap = Record<string, string[]>;

/**
 * 从 NEXUS_TEST_PLAN markdown 文件中解析测试用例
 */
export function parseTestPlan(planDir: string): TestCase[] {
  const cases: TestCase[] = [];
  const files = ['NEXUS_TEST_PLAN.md', 'NEXUS_TEST_PLAN_PART2.md', 'NEXUS_TEST_PLAN_PART3.md'];

  for (const file of files) {
    const filePath = path.join(planDir, file);
    if (!fs.existsSync(filePath)) continue;

    const content = fs.readFileSync(filePath, 'utf-8');
    const lines = content.split('\n');

    let currentModule = '';

    for (const line of lines) {
      // 检测模块标题: ## 1. Entity Registry — 实体注册管理
      const headingMatch = line.match(/^##\s+\d+\.?\s*(.+?)(?:\s*—\s*|\s*$)/);
      if (headingMatch) {
        currentModule = headingMatch[1].trim();
        continue;
      }

      // 子标题也算模块: ### 13.1 卖单流程
      const subHeadingMatch = line.match(/^###\s+[\d.]+\s*(.+)/);
      if (subHeadingMatch) {
        // 保持父模块名
        continue;
      }

      // 解析表格行: | ER-001 | 正常创建... | R2 | 正向 | P0 |
      const tableMatch = line.match(
        /^\|\s*([A-Z]{2,4}-\d{3})\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*(.+?)\s*\|\s*(P[012])\s*\|/
      );
      if (tableMatch) {
        cases.push({
          id: tableMatch[1].trim(),
          module: currentModule,
          description: tableMatch[2].trim(),
          role: tableMatch[3].trim(),
          type: tableMatch[4].trim(),
          priority: tableMatch[5].trim(),
          covered: false,
        });
      }
    }
  }

  return cases;
}

/**
 * 将覆盖映射应用到测试用例列表
 */
export function applyCoverage(cases: TestCase[], coverageMap: CoverageMap): void {
  for (const [flowName, caseIds] of Object.entries(coverageMap)) {
    for (const caseId of caseIds) {
      const tc = cases.find(c => c.id === caseId);
      if (tc) {
        tc.covered = true;
        tc.coveredBy = flowName;
      }
    }
  }
}

/**
 * 生成覆盖率报告
 */
export function generateCoverageReport(cases: TestCase[]): CoverageReport {
  const totalCases = cases.length;
  const coveredCases = cases.filter(c => c.covered).length;

  // 按模块统计
  const byModule: Record<string, { total: number; covered: number; percent: number }> = {};
  for (const c of cases) {
    if (!byModule[c.module]) byModule[c.module] = { total: 0, covered: 0, percent: 0 };
    byModule[c.module].total++;
    if (c.covered) byModule[c.module].covered++;
  }
  for (const m of Object.values(byModule)) {
    m.percent = m.total > 0 ? Math.round((m.covered / m.total) * 100) : 0;
  }

  // 按优先级统计
  const byPriority: Record<string, { total: number; covered: number; percent: number }> = {};
  for (const p of ['P0', 'P1', 'P2']) {
    const pCases = cases.filter(c => c.priority === p);
    const pCovered = pCases.filter(c => c.covered).length;
    byPriority[p] = {
      total: pCases.length,
      covered: pCovered,
      percent: pCases.length > 0 ? Math.round((pCovered / pCases.length) * 100) : 0,
    };
  }

  return {
    totalCases,
    coveredCases,
    coveragePercent: totalCases > 0 ? Math.round((coveredCases / totalCases) * 100) : 0,
    byModule,
    byPriority,
    uncoveredP0: cases.filter(c => !c.covered && c.priority === 'P0'),
    uncoveredP1: cases.filter(c => !c.covered && c.priority === 'P1'),
  };
}

/**
 * 打印覆盖率报告
 */
export function printCoverageReport(report: CoverageReport): void {
  console.log('\n' + '═'.repeat(70));
  console.log('  测试计划覆盖率报告');
  console.log('═'.repeat(70));

  console.log(`\n  总覆盖率: ${report.coveredCases}/${report.totalCases} (${report.coveragePercent}%)`);

  // 按优先级
  console.log('\n  按优先级:');
  for (const [p, stats] of Object.entries(report.byPriority)) {
    const bar = makeBar(stats.percent, 20);
    console.log(`    ${p}: ${bar} ${stats.covered}/${stats.total} (${stats.percent}%)`);
  }

  // 按模块
  console.log('\n  按模块:');
  const modules = Object.entries(report.byModule).sort((a, b) => a[0].localeCompare(b[0]));
  for (const [name, stats] of modules) {
    const bar = makeBar(stats.percent, 15);
    const shortName = name.length > 30 ? name.slice(0, 27) + '...' : name;
    console.log(`    ${shortName.padEnd(32)} ${bar} ${stats.covered}/${stats.total} (${stats.percent}%)`);
  }

  // 未覆盖 P0
  if (report.uncoveredP0.length > 0) {
    console.log(`\n  ⚠ 未覆盖的 P0 用例 (${report.uncoveredP0.length}个):`);
    for (const c of report.uncoveredP0) {
      console.log(`    ${c.id}: ${c.description} [${c.role}]`);
    }
  }

  // 未覆盖 P1 (仅显示数量)
  if (report.uncoveredP1.length > 0) {
    console.log(`\n  ℹ 未覆盖的 P1 用例: ${report.uncoveredP1.length}个`);
  }

  console.log('═'.repeat(70));
}

function makeBar(percent: number, width: number): string {
  const filled = Math.round((percent / 100) * width);
  const empty = width - filled;
  return '█'.repeat(filled) + '░'.repeat(empty);
}

/**
 * 将覆盖率报告写入 JSON 文件
 */
export function writeCoverageJSON(report: CoverageReport, outputPath: string): void {
  fs.writeFileSync(outputPath, JSON.stringify(report, null, 2), 'utf-8');
}
