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
  CoverageMap,
} from './core/coverage-tracker.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const planDir = path.resolve(__dirname, '../docs');

const COVERAGE_MAP: CoverageMap = {
  'Flow-E1: 实体→店铺创建': [
    'ER-001', 'ER-007', 'ER-009', 'ER-011', 'ER-012', 'ER-015', 'ER-016',
    'ER-013', 'ER-014', 'SH-001',
  ],
  'Flow-T1: 做市商生命周期': ['NM-001'],
  'Flow-T2: P2P Buy': ['NM-007', 'NM-008', 'NM-009', 'NM-010'],
  'Flow-T3: P2P Sell': ['NM-001', 'NM-002', 'NM-003', 'NM-005', 'NM-006'],
  'Flow-E2: 订单生命周期': [
    'OD-001', 'OD-004', 'OD-005', 'OD-006', 'OD-007', 'OD-012',
    'OD-013', 'OD-014', 'OD-015', 'SV-001', 'SV-003',
  ],
  'Flow-E3: 会员推荐裂变': [
    'MB-001', 'MB-002', 'MB-003', 'MB-004', 'MB-005', 'MB-007', 'MB-008',
    'MB-011', 'MB-012', 'MB-015',
  ],
  'Flow-D1: 争议解决': [
    'EV-001', 'AR-008', 'AR-009', 'AR-010', 'AR-011', 'AR-012', 'AR-013',
  ],
  'Flow-G1: Bot 生命周期': [
    'GR-001', 'GR-002', 'GR-003', 'GR-004', 'GR-006',
  ],
  'Flow-E5: Token+治理': [
    'TK-001', 'TK-005', 'TK-007', 'TK-011', 'TK-012', 'TK-014',
    'GV-001', 'GV-003', 'GV-004', 'GV-006', 'GV-009', 'GV-011',
  ],
  'Flow-E6: KYC 认证': [
    'KY-001', 'KY-002', 'KY-003', 'KY-004', 'KY-007', 'KY-009', 'KY-010', 'KY-011',
  ],
  'Flow-E4: 佣金返佣': [
    'CM-001', 'CM-002', 'CM-003', 'CM-004', 'CM-009', 'CM-010', 'CM-013',
    'CM-016', 'CM-021',
  ],
  'Flow-E7: 代币发售': [
    'TS-001', 'TS-003', 'TS-004', 'TS-006', 'TS-007', 'TS-009', 'TS-010',
    'TS-012', 'TS-014', 'TS-015', 'TS-016', 'TS-017', 'TS-018',
  ],
  'Flow-G2: 节点共识': [
    'CN-001', 'CN-002', 'CN-003', 'CN-004', 'CN-005', 'CN-006',
    'CN-008', 'CN-009', 'CN-010', 'CN-011',
  ],
  'Flow-G3: 广告活动': [
    'AD-001', 'AD-002', 'AD-003', 'AD-004', 'AD-005', 'AD-007', 'AD-008',
    'AD-009', 'AD-010', 'AD-011', 'AD-012', 'AD-013', 'AD-014',
    'AD-018', 'AD-019', 'AD-020', 'AD-021', 'AD-022',
  ],
  'Flow-S1: 存储服务': [
    'SS-001', 'SS-002', 'SS-003', 'SS-004', 'SS-005', 'SS-006',
    'SS-008', 'SS-010', 'SS-013', 'SS-015', 'SS-016', 'SS-017',
    'SS-018', 'SS-020', 'SS-022',
  ],
};

const cases = parseTestPlan(planDir);
console.log(`解析到 ${cases.length} 个测试用例`);

if (cases.length === 0) {
  console.log('未找到用例，检查 docs/ 目录');
  process.exit(1);
}

applyCoverage(cases, COVERAGE_MAP);
const report = generateCoverageReport(cases);
printCoverageReport(report);
