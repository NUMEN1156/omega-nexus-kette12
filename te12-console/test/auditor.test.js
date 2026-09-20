import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { Auditor } from "../server/auditor.js";
import { WormLog } from "../server/worm.js";

function makeAuditor() {
  const worm = new WormLog({
    filePath: join(mkdtempSync(join(tmpdir(), "te12-auditor-")), "worm.jsonl"),
  });
  return { worm, auditor: new Auditor({ worm }) };
}

test("Auditor cross-validates matching and divergent digests", () => {
  const { worm, auditor } = makeAuditor();
  const digestA = "a".repeat(64);
  const digestB = "b".repeat(64);
  assert.equal(
    auditor.crossValidate({
      tenantA: "A",
      tenantB: "B",
      digestA,
      digestB: digestA,
      subject: "same",
    }).status,
    "VERIFIED",
  );
  assert.equal(
    auditor.crossValidate({
      tenantA: "A",
      tenantB: "B",
      digestA,
      digestB,
      subject: "different",
    }).status,
    "DIVERGENT",
  );
  assert.throws(() =>
    auditor.crossValidate({
      tenantA: "A",
      tenantB: "B",
      digestA: "A".repeat(64),
      digestB,
      subject: "bad",
    }),
  );
  const matrix = auditor.matrix();
  assert.deepEqual(matrix, {
    total: 2,
    verified: 1,
    divergent: 1,
    byPair: { "A|B": { verified: 1, divergent: 1 } },
    recent: matrix.recent,
  });
  assert.equal(
    worm.entries({ tenant: "AUDITOR" }).filter((entry) => entry.kind === "CROSS_VALIDATION").length,
    2,
  );
});
