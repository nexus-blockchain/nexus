/**
 * E2E 测试报告生成器
 */

export interface FlowResult {
  flowName: string;
  description: string;
  steps: StepResult[];
  startTime: number;
  endTime: number;
  passed: boolean;
  error?: string;
}

export interface StepResult {
  name: string;
  actor: string;
  passed: boolean;
  duration: number;
  error?: string;
}

export class TestReporter {
  private flows: FlowResult[] = [];
  private currentFlow: FlowResult | null = null;

  startFlow(name: string, description: string): void {
    this.currentFlow = {
      flowName: name,
      description,
      steps: [],
      startTime: Date.now(),
      endTime: 0,
      passed: true,
    };
  }

  recordStep(name: string, actor: string, passed: boolean, duration: number, error?: string): void {
    if (!this.currentFlow) return;
    this.currentFlow.steps.push({ name, actor, passed, duration, error });
    if (!passed) this.currentFlow.passed = false;
  }

  endFlow(error?: string): FlowResult {
    if (!this.currentFlow) throw new Error('No active flow');
    this.currentFlow.endTime = Date.now();
    if (error) {
      this.currentFlow.passed = false;
      this.currentFlow.error = error;
    }
    const flow = this.currentFlow;
    this.flows.push(flow);
    this.currentFlow = null;
    return flow;
  }

  /** 打印单个 flow 结果 */
  printFlowResult(flow: FlowResult): void {
    const duration = ((flow.endTime - flow.startTime) / 1000).toFixed(1);
    const icon = flow.passed ? '✅' : '❌';
    console.log(`\n${icon} ${flow.flowName} (${duration}s) — ${flow.description}`);

    for (const step of flow.steps) {
      const sIcon = step.passed ? '  ✓' : '  ✗';
      const sDur = `${step.duration}ms`;
      console.log(`${sIcon} [${step.actor}] ${step.name} (${sDur})${step.error ? ` — ${step.error}` : ''}`);
    }

    if (flow.error) {
      console.log(`  错误: ${flow.error}`);
    }
  }

  /** 打印全部汇总 */
  printSummary(): void {
    const total = this.flows.length;
    const passed = this.flows.filter((f) => f.passed).length;
    const failed = total - passed;
    const totalSteps = this.flows.reduce((sum, f) => sum + f.steps.length, 0);
    const passedSteps = this.flows.reduce(
      (sum, f) => sum + f.steps.filter((s) => s.passed).length,
      0,
    );
    const totalTime = this.flows.reduce((sum, f) => sum + (f.endTime - f.startTime), 0);

    console.log('\n' + '='.repeat(70));
    console.log('  E2E 测试汇总');
    console.log('='.repeat(70));
    console.log(`  流程: ${passed}/${total} 通过`);
    console.log(`  步骤: ${passedSteps}/${totalSteps} 通过`);
    console.log(`  耗时: ${(totalTime / 1000).toFixed(1)}s`);
    console.log('-'.repeat(70));

    for (const flow of this.flows) {
      const icon = flow.passed ? '✅' : '❌';
      const dur = ((flow.endTime - flow.startTime) / 1000).toFixed(1);
      const steps = `${flow.steps.filter((s) => s.passed).length}/${flow.steps.length}`;
      console.log(`  ${icon} ${flow.flowName.padEnd(35)} ${steps.padEnd(8)} ${dur}s`);
    }

    console.log('='.repeat(70));
    if (failed > 0) {
      console.log(`\n❌ ${failed} 个流程失败`);
    } else {
      console.log(`\n✅ 全部通过`);
    }
  }

  /** 生成 JSON 报告 */
  toJSON(): string {
    return JSON.stringify(
      {
        timestamp: new Date().toISOString(),
        summary: {
          total: this.flows.length,
          passed: this.flows.filter((f) => f.passed).length,
          failed: this.flows.filter((f) => !f.passed).length,
        },
        flows: this.flows,
      },
      null,
      2,
    );
  }

  get allPassed(): boolean {
    return this.flows.every((f) => f.passed);
  }
}
