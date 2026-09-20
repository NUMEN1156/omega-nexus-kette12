import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname } from "node:path";
import { canonicalJSON, sha3_256Hex } from "./canonical.js";

const GENESIS = "0".repeat(64);
const KINDS = new Set([
  "TENSOR_STEP",
  "TENSOR_APPLY",
  "CROSS_VALIDATION",
  "CHAIN_VERIFY",
  "SYSTEM",
]);

function copyEntry(entry) {
  return JSON.parse(JSON.stringify(entry));
}

export class WormLog {
  constructor({ filePath }) {
    this.filePath = filePath;
    this._entries = [];
    this._seq = 0;
    this._prevHash = GENESIS;
    this._listeners = new Set();
    mkdirSync(dirname(filePath), { recursive: true });
    this.load();
  }

  load() {
    this._entries = [];
    this._seq = 0;
    this._prevHash = GENESIS;
    if (!existsSync(this.filePath)) {
      return;
    }
    const content = readFileSync(this.filePath, "utf8");
    for (const line of content.split("\n")) {
      if (!line.trim()) {
        continue;
      }
      const entry = JSON.parse(line);
      this._entries.push(entry);
    }
    const last = this._entries.at(-1);
    if (last) {
      this._seq = last.seq;
      this._prevHash = last.hash;
    }
  }

  append({ tenant, kind, payload }) {
    if (typeof tenant !== "string" || tenant.length === 0) {
      throw new TypeError("tenant must be a non-empty string");
    }
    if (!KINDS.has(kind)) {
      throw new TypeError(`invalid kind: ${kind}`);
    }
    const seq = this._seq + 1;
    const ts = new Date().toISOString();
    const prevHash = this._prevHash;
    const body = { seq, ts, tenant, kind, payload };
    const hash = sha3_256Hex(prevHash + canonicalJSON(body));
    const entry = { ...body, prevHash, hash };
    appendFileSync(this.filePath, `${canonicalJSON(entry)}\n`, { flag: "a" });
    this._entries.push(entry);
    this._seq = seq;
    this._prevHash = hash;
    for (const listener of this._listeners) {
      try {
        listener(copyEntry(entry));
      } catch {
      }
    }
    return copyEntry(entry);
  }

  entries({ tenant, limit } = {}) {
    let result = this._entries;
    if (tenant !== undefined) {
      result = result.filter((entry) => entry.tenant === tenant);
    }
    if (limit !== undefined) {
      const count = Math.max(0, Math.floor(Number(limit)));
      result = result.slice(-count);
    }
    return result.map(copyEntry);
  }

  verify() {
    let records;
    try {
      const content = existsSync(this.filePath)
        ? readFileSync(this.filePath, "utf8")
        : "";
      records = content
        .split("\n")
        .filter((line) => line.trim())
        .map((line) => JSON.parse(line));
    } catch (error) {
      return {
        ok: false,
        length: 0,
        headHash: null,
        brokenAt: null,
        reason: error.message,
      };
    }

    let previous = GENESIS;
    for (let index = 0; index < records.length; index += 1) {
      const entry = records[index];
      const expectedSeq = index + 1;
      const expectedBody = {
        seq: entry.seq,
        ts: entry.ts,
        tenant: entry.tenant,
        kind: entry.kind,
        payload: entry.payload,
      };
      let expectedHash;
      try {
        expectedHash = sha3_256Hex(previous + canonicalJSON(expectedBody));
      } catch (error) {
        return {
          ok: false,
          length: records.length,
          headHash: records.at(-1)?.hash ?? null,
          brokenAt: entry.seq ?? expectedSeq,
          reason: error.message,
        };
      }
      if (entry.seq !== expectedSeq) {
        return {
          ok: false,
          length: records.length,
          headHash: records.at(-1)?.hash ?? null,
          brokenAt: entry.seq ?? expectedSeq,
          reason: `sequence mismatch at ${entry.seq}`,
        };
      }
      if (entry.prevHash !== previous) {
        return {
          ok: false,
          length: records.length,
          headHash: records.at(-1)?.hash ?? null,
          brokenAt: entry.seq,
          reason: `previous hash mismatch at ${entry.seq}`,
        };
      }
      if (entry.hash !== expectedHash) {
        return {
          ok: false,
          length: records.length,
          headHash: records.at(-1)?.hash ?? null,
          brokenAt: entry.seq,
          reason: `hash mismatch at ${entry.seq}`,
        };
      }
      previous = entry.hash;
    }
    return {
      ok: true,
      length: records.length,
      headHash: records.at(-1)?.hash ?? null,
      brokenAt: null,
      reason: null,
    };
  }

  head() {
    return this._entries.at(-1)?.hash ?? null;
  }

  onAppend(listener) {
    this._listeners.add(listener);
    return () => this._listeners.delete(listener);
  }
}
