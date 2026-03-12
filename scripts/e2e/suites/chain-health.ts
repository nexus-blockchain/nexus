import { readFreeBalance } from '../framework/accounts.js';
import { assert } from '../framework/assert.js';
import { TestSuite } from '../framework/types.js';
import { formatNex } from '../framework/units.js';

export const chainHealthSuite: TestSuite = {
  id: 'chain-health',
  title: 'Chain health',
  description: 'Basic connectivity, finalized head visibility, and dev-account funding readiness.',
  tags: ['smoke', 'system'],
  async run(ctx) {
    await ctx.step('runtime snapshot is readable', async () => {
      assert(ctx.chain.chain.length > 0, 'Chain name should not be empty');
      assert(ctx.chain.specVersion > 0, 'Spec version should be positive');
      ctx.note(`connected to ${ctx.chain.chain} / ${ctx.chain.nodeName} ${ctx.chain.nodeVersion}`);
    });

    await ctx.step('finalized head is advancing', async () => {
      const finalizedHead = await ctx.api.rpc.chain.getFinalizedHead();
      const header = await ctx.api.rpc.chain.getHeader(finalizedHead);
      assert(header.number.toNumber() > 0, 'Finalized block number should be > 0');
      ctx.note(`finalized block #${header.number.toString()}`);
    });

    await ctx.step('dev actors can be funded from Alice', async () => {
      await ctx.ensureFunds(25_000);
      const alice = await readFreeBalance(ctx.api, ctx.actors.alice.address);
      const bob = await readFreeBalance(ctx.api, ctx.actors.bob.address);
      assert(alice > 0n, 'Alice should have free balance');
      assert(bob > 0n, 'Bob should have free balance after funding');
      ctx.note(`alice=${formatNex(alice)} bob=${formatNex(bob)}`);
    });
  },
};
