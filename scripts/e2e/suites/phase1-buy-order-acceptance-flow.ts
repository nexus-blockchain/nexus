import { submitTx } from '../framework/api.js';
import { assert, assertEvent, assertTxSuccess } from '../framework/assert.js';
import { readObjectField } from '../framework/codec.js';
import { TestSuite } from '../framework/types.js';
import { nex } from '../framework/units.js';
import {
  VALID_TRON_ADDRESSES,
  describeMarketField,
  findRecentMakerOrder,
  marketFieldContains,
  readMarketOrder,
  readMarketTrade,
  readNextMarketOrderId,
  readNextUsdtTradeId,
  readOrderTrades,
  readSafeMarketPrices,
  readUserOrders,
  readUserTrades,
} from './market-helpers.js';

const ORDER_AMOUNT = nex(10).toString();

export const phase1BuyOrderAcceptanceFlowSuite: TestSuite = {
  id: 'phase1-buy-order-acceptance-flow',
  title: 'Phase 1 / S1-04 buy order acceptance flow',
  description: 'Verify placeBuyOrder → acceptBuyOrder → confirmPayment → sellerConfirmReceived completes a buy-side NEX trade.',
  tags: ['phase1', 'market', 'buy-order'],
  async run(ctx) {
    const seller = ctx.actors.dave;
    const buyer = ctx.actors.alice;
    const tx = ctx.api.tx as any;

    await ctx.step('fund the seller and resolve safe market prices', async () => {
      await ctx.ensureFundsFor(['dave'], 25_000);
      const marketPrice = await ctx.readMarketPrice();
      assert(marketPrice > 0, 'market price should be positive');
      const prices = await readSafeMarketPrices(ctx.api, marketPrice);
      ctx.note(`marketPrice=${marketPrice} buyPrice=${prices.buyPrice} sellPrice=${prices.sellPrice}`);
      return prices;
    });

    const prices = await readSafeMarketPrices(ctx.api, await ctx.readMarketPrice());

    const buyOrderId = await ctx.step('buyer places a buy order that remains open for acceptance', async () => {
      const beforeNextOrderId = await readNextMarketOrderId(ctx.api);
      const beforeOrders = await readUserOrders(ctx.api, buyer.address);

      const receipt = await submitTx(
        ctx.api,
        tx.nexMarket.placeBuyOrder(ORDER_AMOUNT, prices.buyPrice, VALID_TRON_ADDRESSES.buyer),
        buyer,
        'place buy order',
      );
      assertTxSuccess(receipt, 'placeBuyOrder should succeed');
      assertEvent(receipt, 'nexMarket', 'OrderCreated', 'placeBuyOrder should emit OrderCreated');

      const afterNextOrderId = await readNextMarketOrderId(ctx.api);
      const orderId = await findRecentMakerOrder(ctx.api, buyer.address, 'buy', beforeNextOrderId, afterNextOrderId);
      assert(orderId != null, `expected a new buy order between ids [${beforeNextOrderId}, ${afterNextOrderId})`);

      const order = await readMarketOrder(ctx.api, orderId);
      const status = describeMarketField(order, 'status');
      const side = describeMarketField(order, 'side');
      assert(side.toLowerCase().includes('buy'), `buy order side should be Buy, got ${side}`);
      assert(
        marketFieldContains(order, 'status', 'open') || marketFieldContains(order, 'status', 'fill'),
        `new buy order should be open/fill-like, got ${status}`,
      );

      if (beforeOrders.length >= 0) {
        ctx.note(`buyOrderId=${orderId} status=${status}`);
      }
      return orderId;
    });

    const tradeId = await ctx.step('seller accepts the buy order', async () => {
      const beforeTradeId = await readNextUsdtTradeId(ctx.api);
      const receipt = await submitTx(
        ctx.api,
        tx.nexMarket.acceptBuyOrder(buyOrderId, null, VALID_TRON_ADDRESSES.seller),
        seller,
        'accept buy order',
      );
      assertTxSuccess(receipt, 'acceptBuyOrder should succeed');
      assertEvent(receipt, 'nexMarket', 'UsdtTradeCreated', 'acceptBuyOrder should emit UsdtTradeCreated');
      assertEvent(receipt, 'nexMarket', 'BuyerDepositLocked', 'acceptBuyOrder should lock buyer deposit');

      const trade = await readMarketTrade(ctx.api, beforeTradeId);
      const status = describeMarketField(trade, 'status').toLowerCase();
      assert(status.includes('awaitingpayment'), `accepted buy trade should await payment, got ${status}`);
      assertEqualAddress(String(readObjectField(trade.json, 'buyer')), buyer.address, 'trade buyer should match buy-order maker');
      assertEqualAddress(String(readObjectField(trade.json, 'seller')), seller.address, 'trade seller should match acceptor');
      return beforeTradeId;
    });

    await ctx.step('buyer confirms payment and seller finalizes the trade', async () => {
      const confirmReceipt = await submitTx(
        ctx.api,
        tx.nexMarket.confirmPayment(tradeId),
        buyer,
        'confirm buy-order payment',
      );
      assertTxSuccess(confirmReceipt, 'confirmPayment should succeed on accepted buy order');
      assertEvent(confirmReceipt, 'nexMarket', 'UsdtPaymentSubmitted', 'confirmPayment should emit UsdtPaymentSubmitted');

      const finalizeReceipt = await submitTx(
        ctx.api,
        tx.nexMarket.sellerConfirmReceived(tradeId),
        seller,
        'seller confirm received',
      );
      assertTxSuccess(finalizeReceipt, 'sellerConfirmReceived should succeed on accepted buy order');
      assertEvent(finalizeReceipt, 'nexMarket', 'UsdtTradeCompleted', 'sellerConfirmReceived should emit UsdtTradeCompleted');
      assertEvent(finalizeReceipt, 'nexMarket', 'SellerConfirmedReceived', 'sellerConfirmReceived should emit SellerConfirmedReceived');

      const trade = await readMarketTrade(ctx.api, tradeId);
      const order = await readMarketOrder(ctx.api, buyOrderId);
      const orderTrades = await readOrderTrades(ctx.api, buyOrderId);
      const buyerTrades = await readUserTrades(ctx.api, buyer.address);
      const sellerTrades = await readUserTrades(ctx.api, seller.address);
      const tradeStatus = describeMarketField(trade, 'status').toLowerCase();
      const orderStatus = describeMarketField(order, 'status').toLowerCase();
      const depositStatus = describeMarketField(trade, 'depositStatus').toLowerCase();

      assert(tradeStatus.includes('completed'), `trade should be Completed, got ${tradeStatus}`);
      assert(orderStatus.includes('fill'), `buy order should be filled after acceptance flow, got ${orderStatus}`);
      assert(depositStatus.includes('released'), `buyer deposit should be released after completion, got ${depositStatus}`);
      assert(orderTrades.includes(tradeId), `order ${buyOrderId} should index trade ${tradeId}`);
      assert(buyerTrades.includes(tradeId), `buyer trade index should contain trade ${tradeId}`);
      assert(sellerTrades.includes(tradeId), `seller trade index should contain trade ${tradeId}`);
      ctx.note(`buyOrderId=${buyOrderId} tradeId=${tradeId} finalTradeStatus=${tradeStatus}`);
    });
  },
};

function assertEqualAddress(actual: string, expected: string, message: string): void {
  assert(actual === expected, `${message}: expected=${expected} actual=${actual}`);
}
