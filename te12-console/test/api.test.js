import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { createServer } from "../server/index.js";

let server;
let base;

test.before(async () => {
  const dataDir = mkdtempSync(join(tmpdir(), "te12-api-"));
  server = createServer({ dataDir });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  base = `http://127.0.0.1:${server.address().port}`;
});

test.after(async () => {
  await new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve())));
});

test("TE12 API serves state, tensor, auditor, verification, and 404s", async () => {
  let response = await fetch(`${base}/api/state`);
  assert.equal(response.status, 200);
  let body = await response.json();
  assert.equal(body.worm.length, 1);

  response = await fetch(`${base}/api/tensor/step`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ tenant: "A" }),
  });
  assert.equal(response.status, 200);
  body = await response.json();
  assert.equal(body.entry.kind, "TENSOR_STEP");

  response = await fetch(`${base}/api/tensor/apply`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ matrix: [[1]] }),
  });
  assert.equal(response.status, 400);

  response = await fetch(`${base}/api/worm/verify`);
  assert.equal(response.status, 200);
  body = await response.json();
  assert.equal(body.verify.ok, true);

  response = await fetch(`${base}/api/auditor/cross-validate`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      tenantA: "A",
      tenantB: "B",
      digestA: "a".repeat(64),
      digestB: "a".repeat(64),
      subject: "api",
    }),
  });
  assert.equal(response.status, 200);
  assert.equal((await response.json()).status, "VERIFIED");

  assert.equal((await fetch(`${base}/nonexistent`)).status, 404);
  assert.ok([403, 404].includes((await fetch(`${base}/%2e%2e/package.json`)).status));
});
