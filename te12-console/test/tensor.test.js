import assert from "node:assert/strict";
import test from "node:test";
import { PredictiveTensorController } from "../server/tensor.js";

test("PredictiveTensorController is deterministic and seedable", () => {
  const first = new PredictiveTensorController({ dims: 8, seed: 12 });
  const second = new PredictiveTensorController({ dims: 8, seed: 12 });
  for (let n = 0; n < 5; n += 1) {
    assert.deepEqual(first.step(), second.step());
  }
  const different = new PredictiveTensorController({ dims: 8, seed: 13 });
  assert.notEqual(first.snapshot().digest, different.step().digest);
  assert.ok(Math.abs(first.snapshot().state.reduce((a, b) => a + b, 0) - 1) < 1e-9);
  assert.ok(first.snapshot().entropy > 0);
  assert.ok(first.snapshot().entropy < Math.log2(8));
});

test("apply validates matrices and preserves identity state", () => {
  const tensor = new PredictiveTensorController({ dims: 3, seed: 12 });
  const before = tensor.snapshot().state;
  const identity = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
  ];
  const after = tensor.apply(identity);
  after.state.forEach((value, index) => {
    assert.ok(Math.abs(value - before[index]) < 1e-12);
  });
  assert.throws(() => tensor.apply([[1, 0], [0, 1]]));
  assert.throws(() => tensor.apply([[1, 0, 0], [0, 1, 0], [0, 0, Infinity]]));
});
