#!/usr/bin/env node

import { createHash } from 'node:crypto';

const DEFAULT_REMOTE_WS_URL = 'wss://202.140.140.202';
const DEFAULT_BLOCK_NUMBER = '0';

function printHelp() {
  console.log(`
Usage:
  node query-genesis-inscription.js [options]

Options:
  --ws <url>         WebSocket endpoint (default: ${DEFAULT_REMOTE_WS_URL})
  --block <number>   Block number to query first (default: ${DEFAULT_BLOCK_NUMBER})
  --json             Print JSON only
  --insecure-tls     Disable TLS certificate verification for this process
  --strict-tls       Keep TLS certificate verification enabled
  -h, --help         Show this help

Examples:
  node query-genesis-inscription.js
  node query-genesis-inscription.js --block 0 --json
  node query-genesis-inscription.js --ws ws://127.0.0.1:9944 --strict-tls
`.trim());
}

function parseArgs(argv) {
  let wsUrl = process.env.WS_URL ?? DEFAULT_REMOTE_WS_URL;
  let blockNumber = DEFAULT_BLOCK_NUMBER;
  let json = false;
  let help = false;
  let insecureTls = process.env.NODE_TLS_REJECT_UNAUTHORIZED === '0';
  let tlsModeExplicit = false;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    switch (arg) {
      case '--ws': {
        const value = argv[index + 1];
        if (!value) {
          throw new Error('Missing value for --ws');
        }
        wsUrl = value;
        index += 1;
        break;
      }
      case '--block': {
        const value = argv[index + 1];
        if (!value) {
          throw new Error('Missing value for --block');
        }
        if (!/^\d+$/.test(value)) {
          throw new Error(`Invalid block number: ${value}`);
        }
        blockNumber = value;
        index += 1;
        break;
      }
      case '--json':
        json = true;
        break;
      case '--insecure-tls':
        insecureTls = true;
        tlsModeExplicit = true;
        break;
      case '--strict-tls':
        insecureTls = false;
        tlsModeExplicit = true;
        break;
      case '-h':
      case '--help':
        help = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!tlsModeExplicit && wsUrl === DEFAULT_REMOTE_WS_URL) {
    insecureTls = true;
  }

  return {
    wsUrl,
    blockNumber,
    json,
    insecureTls,
    help,
  };
}

function formatError(error) {
  return error instanceof Error ? error.message : String(error);
}

function isStatePrunedError(error) {
  const message = formatError(error).toLowerCase();
  return message.includes('state already discarded') || message.includes('unknown block');
}

function codecToHex(value) {
  if (value && typeof value.toHex === 'function') {
    return value.toHex();
  }
  throw new Error('Codec does not support toHex()');
}

function hexToBuffer(hex) {
  const normalized = hex.startsWith('0x') ? hex.slice(2) : hex;
  return Buffer.from(normalized, 'hex');
}

function decodeUtf8(hex) {
  return hexToBuffer(hex).toString('utf8');
}

function sha256Hex(bytes) {
  return `0x${createHash('sha256').update(bytes).digest('hex')}`;
}

function getInscriptionQueries(api) {
  const inscriptionSection = api.query.inscription;

  if (!inscriptionSection?.inscription || !inscriptionSection?.inscriptionHash) {
    throw new Error('Chain metadata does not expose query.inscription.inscription / inscriptionHash');
  }

  return {
    inscription: () => inscriptionSection.inscription(),
    inscriptionHash: () => inscriptionSection.inscriptionHash(),
    inscriptionAt: (hash) => inscriptionSection.inscription.at(hash),
    inscriptionHashAt: (hash) => inscriptionSection.inscriptionHash.at(hash),
  };
}

function printHumanReadable(result) {
  console.log(`Endpoint: ${result.url}`);
  console.log(
    `Chain: ${result.chain.chain} / ${result.chain.specName}@${result.chain.specVersion} (${result.chain.nodeName} ${result.chain.nodeVersion})`,
  );
  console.log(`Requested block: #${result.requestedBlock.number} (${result.requestedBlock.hash})`);
  console.log(`Finalized head: #${result.finalizedHead.number} (${result.finalizedHead.hash})`);
  console.log(`Data source: ${result.inscription.source}`);
  console.log(`On-chain hash: ${result.inscription.hashOnChain}`);
  console.log(`Computed SHA-256: ${result.inscription.hashComputedFromBytes}`);
  console.log(`Hash matches: ${result.inscription.hashMatches ? 'yes' : 'no'}`);

  if (result.requestedBlock.historicalRead.message) {
    console.log(`Historical read: ${result.requestedBlock.historicalRead.message}`);
  }

  if (result.notes.length > 0) {
    console.log('Notes:');
    for (const note of result.notes) {
      console.log(`- ${note}`);
    }
  }

  console.log('\nInscription:\n');
  console.log(result.inscription.utf8);
}

async function main() {
  const options = parseArgs(process.argv.slice(2));

  if (options.help) {
    printHelp();
    return;
  }

  process.env.POLKADOTJS_DISABLE_ESM_CJS_WARNING = '1';

  if (options.insecureTls) {
    process.env.NODE_TLS_REJECT_UNAUTHORIZED ??= '0';
  }

  const { ApiPromise, WsProvider } = await import('@polkadot/api');
  const api = await ApiPromise.create({
    provider: new WsProvider(options.wsUrl),
    noInitWarn: true,
  });

  try {
    const queries = getInscriptionQueries(api);

    const [chainName, nodeName, nodeVersion, finalizedHead, requestedBlockHash] = await Promise.all([
      api.rpc.system.chain(),
      api.rpc.system.name(),
      api.rpc.system.version(),
      api.rpc.chain.getFinalizedHead(),
      api.rpc.chain.getBlockHash(options.blockNumber),
    ]);

    const finalizedHeader = await api.rpc.chain.getHeader(finalizedHead);
    const [currentInscription, currentInscriptionHash] = await Promise.all([
      queries.inscription(),
      queries.inscriptionHash(),
    ]);

    let selectedInscription = currentInscription;
    let selectedInscriptionHash = currentInscriptionHash;
    let source = 'current_finalized_head';
    const notes = [];

    let historicalRead = {
      ok: false,
      source: 'current_finalized_head',
      message: 'Historical query not attempted',
    };

    try {
      const [historicalInscription, historicalInscriptionHash] = await Promise.all([
        queries.inscriptionAt(requestedBlockHash.toHex()),
        queries.inscriptionHashAt(requestedBlockHash.toHex()),
      ]);

      selectedInscription = historicalInscription;
      selectedInscriptionHash = historicalInscriptionHash;
      source = 'requested_block';
      historicalRead = {
        ok: true,
        source,
        message: `Historical query succeeded at block #${options.blockNumber}`,
      };
    } catch (error) {
      historicalRead = {
        ok: false,
        source: 'current_finalized_head',
        message: isStatePrunedError(error)
          ? `Historical state for block #${options.blockNumber} is pruned on this node; fell back to current finalized state`
          : `Historical query failed for block #${options.blockNumber}; fell back to current finalized state: ${formatError(error)}`,
      };

      if (isStatePrunedError(error)) {
        notes.push('This node has pruned early state, so block-specific storage reads may be unavailable.');
      }
    }

    notes.push('Local pallet source marks inscription storage as genesis-only and read-only after genesis.');

    const inscriptionHex = codecToHex(selectedInscription);
    const inscriptionBytes = hexToBuffer(inscriptionHex);
    const inscriptionHashHex = codecToHex(selectedInscriptionHash);
    const computedHashHex = sha256Hex(inscriptionBytes);

    const result = {
      url: options.wsUrl,
      chain: {
        chain: chainName.toString(),
        nodeName: nodeName.toString(),
        nodeVersion: nodeVersion.toString(),
        specName: api.runtimeVersion.specName.toString(),
        specVersion: api.runtimeVersion.specVersion.toString(),
      },
      requestedBlock: {
        number: options.blockNumber,
        hash: requestedBlockHash.toHex(),
        historicalRead,
      },
      finalizedHead: {
        number: finalizedHeader.number.toString(),
        hash: finalizedHead.toHex(),
      },
      inscription: {
        source,
        utf8: decodeUtf8(inscriptionHex),
        hex: inscriptionHex,
        hashOnChain: inscriptionHashHex,
        hashComputedFromBytes: computedHashHex,
        hashMatches: inscriptionHashHex.toLowerCase() === computedHashHex.toLowerCase(),
      },
      notes,
    };

    if (options.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }

    printHumanReadable(result);
  } finally {
    await api.disconnect();
  }
}

await main();
