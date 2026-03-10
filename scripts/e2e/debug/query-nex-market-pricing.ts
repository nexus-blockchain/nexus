import { ApiPromise, WsProvider } from '@polkadot/api';

async function main(): Promise<void> {
  const api = await ApiPromise.create({ provider: new WsProvider('ws://127.0.0.1:9944') });
  try {
    const [header, lastTradePrice, priceProtection, twapAccumulator] = await Promise.all([
      api.rpc.chain.getHeader(),
      (api.query as any).nexMarket.lastTradePrice(),
      (api.query as any).nexMarket.priceProtectionStore(),
      (api.query as any).nexMarket.twapAccumulatorStore(),
    ]);

    const snapshot = {
      head: header.number.toString(),
      lastTradePrice: {
        human: lastTradePrice.toHuman(),
        raw: lastTradePrice.toString(),
      },
      priceProtection: {
        human: priceProtection.toHuman(),
        raw: priceProtection.toJSON(),
      },
      twapAccumulator: {
        human: twapAccumulator.toHuman(),
        raw: twapAccumulator.toJSON(),
      },
    };

    console.log(JSON.stringify(snapshot, null, 2));
  } finally {
    await api.disconnect();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
