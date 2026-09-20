import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { WormLog } from "../server/worm.js";

function makeWorm() {
  return new WormLog({
    filePath: join(mkdtempSync(join(tmpdir(), "te12-worm-")), "worm.jsonl"),
  });
}

test("WormLog appends and verifies a hash chain", () => {
  const worm = makeWorm();
  worm.append({ tenant: "A", kind: "SYSTEM", payload: { n: 1 } });
  worm.append({ tenant: "A", kind: "SYSTEM", payload: { n: 2 } });
  worm.append({ tenant: "B", kind: "SYSTEM", payload: { n: 3 } });
  const result = worm.verify();
  assert.equal(result.ok, true);
  assert.equal(result.length, 3);
  assert.equal(result.headHash, worm.head());
  assert.equal(worm.entries({ tenant: "A" }).length, 2);
});

test("WormLog detects tampering and reloads state", () => {
  const worm = makeWorm();
  for (let n = 1; n <= 3; n += 1) {
    worm.append({ tenant: "A", kind: "SYSTEM", payload: { n } });
  }
  const lines = readFileSync(worm.filePath, "utf8").trimEnd().split("\n");
  const tampered = JSON.parse(lines[1]);
  tampered.payload.n = 99;
  lines[1] = JSON.stringify(tampered);
  writeFileSync(worm.filePath, `${lines.join("\n")}\n`);
  const result = worm.verify();
  assert.equal(result.ok, false);
  assert.equal(result.brokenAt, 2);
  const reloaded = new WormLog({ filePath: worm.filePath });
  assert.equal(reloaded.entries().length, 3);
  assert.equal(reloaded.head(), worm.entries().at(-1).hash);
  assert.equal(reloaded.append({ tenant: "A", kind: "SYSTEM", payload: {} }).seq, 4);
});

test("WormLog rejects invalid kinds", () => {
  const worm = makeWorm();
  assert.throws(() => worm.append({ tenant: "A", kind: "NOPE", payload: {} }));
});
