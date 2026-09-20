import assert from "node:assert/strict";
import test from "node:test";
import { canonicalJSON, sha3_256Hex } from "../server/canonical.js";

test("canonicalJSON sorts keys recursively and preserves array order", () => {
  assert.equal(
    canonicalJSON({ z: 1, a: { y: 2, x: 3 }, list: [{ b: 1, a: 2 }] }),
    '{"a":{"x":3,"y":2},"list":[{"a":2,"b":1}],"z":1}',
  );
});

test("canonicalJSON rejects non-finite values", () => {
  assert.throws(() => canonicalJSON({ value: Number.NaN }));
  assert.throws(() => canonicalJSON({ value: undefined }));
});

test("sha3_256Hex hashes with SHA3-256", () => {
  assert.equal(
    sha3_256Hex("abc"),
    "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
  );
});
