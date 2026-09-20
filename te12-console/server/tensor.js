import { canonicalJSON, sha3_256Hex } from "./canonical.js";

export function xorshift32(seed) {
  let state = (seed >>> 0) || 0x6d2b79f5;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0x100000000;
  };
}

function normalize(state) {
  const sum = state.reduce((total, value) => total + value, 0);
  return state.map((value) => value / sum);
}

function entropy(state) {
  return -state.reduce(
    (total, value) => total + (value > 0 ? value * Math.log2(value) : 0),
    0,
  );
}

export class PredictiveTensorController {
  constructor({ dims = 8, seed = 12 } = {}) {
    if (!Number.isInteger(dims) || dims < 1) {
      throw new Error("dims must be a positive integer");
    }
    this.dims = dims;
    this.trajectory = [];
    this.reset(seed);
  }

  reset(seed) {
    if (!Number.isFinite(seed)) {
      throw new Error("seed must be finite");
    }
    this.seed = seed;
    this.rng = xorshift32(seed);
    const initial = Array.from({ length: this.dims }, () => this.rng());
    this.state = normalize(
      initial.some((value) => value > 0)
        ? initial
        : Array(this.dims).fill(1),
    );
    this.tick = 0;
    this.trajectory = [];
    return this.snapshot();
  }

  snapshot() {
    const state = [...this.state];
    const value = entropy(state);
    const digest = sha3_256Hex(
      canonicalJSON({
        tick: this.tick,
        state: state.map((item) => item.toFixed(12)),
      }),
    );
    return { tick: this.tick, state, entropy: value, digest };
  }

  record(snapshot) {
    this.trajectory.push({ tick: snapshot.tick, entropy: snapshot.entropy });
    if (this.trajectory.length > 200) {
      this.trajectory.splice(0, this.trajectory.length - 200);
    }
    return snapshot;
  }

  step() {
    const next = this.state.map(
      (value, index) =>
        0.7 * value +
        0.3 * this.state[(index + 1) % this.dims] +
        0.02 * (this.rng() - 0.5),
    );
    this.state = normalize(next.map((value) => Math.max(1e-9, value)));
    this.tick += 1;
    return this.record(this.snapshot());
  }

  apply(matrix) {
    if (!Array.isArray(matrix) || matrix.length !== this.dims) {
      throw new Error(`matrix must have ${this.dims} rows`);
    }
    for (const row of matrix) {
      if (
        !Array.isArray(row) ||
        row.length !== this.dims ||
        row.some((value) => typeof value !== "number" || !Number.isFinite(value))
      ) {
        throw new Error(`matrix must be ${this.dims}x${this.dims} finite numbers`);
      }
    }
    const next = matrix.map((row) =>
      row.reduce((sum, value, index) => sum + value * this.state[index], 0),
    );
    this.state = normalize(next.map((value) => Math.max(1e-9, Math.abs(value))));
    this.tick += 1;
    return this.record(this.snapshot());
  }
}
