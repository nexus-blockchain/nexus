import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
for (const mod of ['@polkadot/api','@polkadot/keyring','@polkadot/util-crypto']) {
  try {
    const main = require.resolve(mod);
    const root = main.split('/cjs/')[0];
    const pkg = JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8'));
    console.log(mod, 'main=', main, 'root=', root, 'version=' + pkg.version);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    console.log(mod, 'ERR', message);
  }
}
