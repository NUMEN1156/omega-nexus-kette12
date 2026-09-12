#!/usr/bin/env node
// WebSocket log listener for the Anvil E2E pipeline.
//
// Subscribes to `logs` on an EVM node via eth_subscribe and prints one JSON
// line per matching event. Exits 0 once every expected event name has been
// observed, exits 1 on timeout or transport failure (fail-closed).
//
// Usage:
//   node scripts/ws-event-listener.mjs --ws ws://127.0.0.1:8545 \
//     --address 0x... --expect Queued,Executed --timeout 60 --ready-file /tmp/ready

import { writeFileSync } from "node:fs";

const EVENTS = {
  Queued: "Queued(uint256,bytes32,uint256,address,bytes)",
  Executed: "Executed(uint256,bytes32,address)",
  Cancelled: "Cancelled(uint256,bytes32,address)",
};

// Minimal keccak-256 (Keccak-f[1600], 0x01 padding) for event topic hashing;
// node:crypto only ships SHA3, which uses different padding.
function keccak(text) {
  const bytes = new TextEncoder().encode(text);
  const RC = [
    1n, 0x8082n, 0x800000000000808an, 0x8000000080008000n, 0x808bn, 0x80000001n,
    0x8000000080008081n, 0x8000000000008009n, 0x8an, 0x88n, 0x80008009n, 0x8000000an,
    0x8000808bn, 0x800000000000008bn, 0x8000000000008089n, 0x8000000000008003n,
    0x8000000000008002n, 0x8000000000000080n, 0x800an, 0x800000008000000an,
    0x8000000080008081n, 0x8000000000008080n, 0x80000001n, 0x8000000080008008n,
  ];
  const ROT = [1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44];
  const PI = [10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1];
  const M = (1n << 64n) - 1n;
  const rotl = (x, n) => ((x << BigInt(n)) | (x >> BigInt(64 - n))) & M;
  const state = new Array(25).fill(0n);
  const rate = 136;
  const padded = new Uint8Array(Math.ceil((bytes.length + 1) / rate) * rate);
  padded.set(bytes);
  padded[bytes.length] ^= 0x01;
  padded[padded.length - 1] ^= 0x80;
  for (let off = 0; off < padded.length; off += rate) {
    for (let i = 0; i < rate / 8; i++) {
      let lane = 0n;
      for (let b = 7; b >= 0; b--) lane = (lane << 8n) | BigInt(padded[off + i * 8 + b]);
      state[i] ^= lane;
    }
    for (let round = 0; round < 24; round++) {
      const c = [];
      for (let x = 0; x < 5; x++) c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
      for (let x = 0; x < 5; x++) {
        const d = c[(x + 4) % 5] ^ rotl(c[(x + 1) % 5], 1);
        for (let y = 0; y < 25; y += 5) state[x + y] ^= d;
      }
      let cur = state[1];
      for (let i = 0; i < 24; i++) {
        const j = PI[i];
        const tmp = state[j];
        state[j] = rotl(cur, ROT[i]);
        cur = tmp;
      }
      for (let y = 0; y < 25; y += 5) {
        const row = state.slice(y, y + 5);
        for (let x = 0; x < 5; x++) state[y + x] = row[x] ^ (~row[(x + 1) % 5] & M & row[(x + 2) % 5]);
      }
      state[0] ^= RC[round];
    }
  }
  let out = "0x";
  for (let i = 0; i < 4; i++) {
    let lane = state[i];
    for (let b = 0; b < 8; b++) {
      out += (lane & 0xffn).toString(16).padStart(2, "0");
      lane >>= 8n;
    }
  }
  return out;
}

function parseArgs(argv) {
  const args = { ws: "ws://127.0.0.1:8545", expect: "Queued,Executed", timeout: 60 };
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i].replace(/^--/, "");
    args[key] = argv[i + 1];
  }
  args.timeout = Number(args.timeout);
  if (!args.address || !/^0x[0-9a-fA-F]{40}$/.test(args.address)) {
    throw new Error("--address must be a 20-byte hex address");
  }
  args.expect = args.expect.split(",").map((s) => s.trim()).filter(Boolean);
  for (const name of args.expect) {
    if (!EVENTS[name]) throw new Error(`unknown event ${name}; known: ${Object.keys(EVENTS).join(",")}`);
  }
  return args;
}

const args = parseArgs(process.argv.slice(2));
const topicToName = new Map(Object.entries(EVENTS).map(([name, sig]) => [keccak(sig), name]));
const remaining = new Set(args.expect);
const seen = [];

const socket = new WebSocket(args.ws);
const deadline = setTimeout(() => {
  console.error(JSON.stringify({ level: "error", msg: "timeout", missing: [...remaining], seen }));
  process.exit(1);
}, args.timeout * 1000);

socket.addEventListener("error", (event) => {
  console.error(JSON.stringify({ level: "error", msg: "websocket error", detail: String(event.message ?? event) }));
  process.exit(1);
});

socket.addEventListener("close", () => {
  if (remaining.size > 0) {
    console.error(JSON.stringify({ level: "error", msg: "websocket closed early", missing: [...remaining] }));
    process.exit(1);
  }
});

socket.addEventListener("open", () => {
  socket.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "eth_subscribe", params: ["logs", { address: args.address }] }));
});

socket.addEventListener("message", (event) => {
  const body = JSON.parse(String(event.data));
  if (body.id === 1) {
    if (body.error) {
      console.error(JSON.stringify({ level: "error", msg: "eth_subscribe rejected", detail: body.error }));
      process.exit(1);
    }
    console.log(JSON.stringify({ level: "info", msg: "subscribed", subscription: body.result, address: args.address }));
    if (args["ready-file"]) writeFileSync(args["ready-file"], body.result);
    return;
  }
  if (body.method !== "eth_subscription") return;
  const log = body.params.result;
  const name = topicToName.get(log.topics[0]) ?? "unknown";
  const record = {
    level: "event",
    name,
    id: name === "unknown" ? null : BigInt(log.topics[1]).toString(),
    payloadHash: log.topics[2] ?? null,
    blockNumber: Number(log.blockNumber),
    txHash: log.transactionHash,
  };
  console.log(JSON.stringify(record));
  seen.push(name);
  remaining.delete(name);
  if (remaining.size === 0) {
    clearTimeout(deadline);
    console.log(JSON.stringify({ level: "info", msg: "all expected events observed", seen }));
    socket.close();
    process.exit(0);
  }
});
