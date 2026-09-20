import { createHash } from "node:crypto";

function normalize(value) {
  if (value === undefined) {
    throw new TypeError("undefined is not valid canonical JSON");
  }
  if (typeof value === "number" && !Number.isFinite(value)) {
    throw new TypeError("non-finite numbers are not valid canonical JSON");
  }
  if (Array.isArray(value)) {
    return value.map(normalize);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, normalize(value[key])]),
    );
  }
  return value;
}

export function canonicalJSON(value) {
  return JSON.stringify(normalize(value));
}

export function sha3_256Hex(str) {
  return createHash("sha3-256").update(str).digest("hex");
}
