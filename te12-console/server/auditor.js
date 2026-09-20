import { canonicalJSON, sha3_256Hex } from "./canonical.js";

const DIGEST = /^[0-9a-f]{64}$/;

export class Auditor {
  constructor({ worm }) {
    this.worm = worm;
  }

  crossValidate({ tenantA, tenantB, digestA, digestB, subject }) {
    if (!DIGEST.test(digestA) || !DIGEST.test(digestB)) {
      throw new Error("digests must be 64-character lowercase hexadecimal strings");
    }
    const result = {
      subject,
      tenantA,
      tenantB,
      status: digestA === digestB ? "VERIFIED" : "DIVERGENT",
      digestA,
      digestB,
      matrixHash: sha3_256Hex(
        canonicalJSON({ tenantA, tenantB, digestA, digestB, subject }),
      ),
    };
    this.worm.append({
      tenant: "AUDITOR",
      kind: "CROSS_VALIDATION",
      payload: result,
    });
    return result;
  }

  matrix() {
    const results = this.worm
      .entries({ tenant: "AUDITOR" })
      .filter((entry) => entry.kind === "CROSS_VALIDATION")
      .map((entry) => entry.payload);
    const byPair = {};
    let verified = 0;
    let divergent = 0;
    for (const result of results) {
      const pair = `${result.tenantA}|${result.tenantB}`;
      byPair[pair] ??= { verified: 0, divergent: 0 };
      if (result.status === "VERIFIED") {
        verified += 1;
        byPair[pair].verified += 1;
      } else {
        divergent += 1;
        byPair[pair].divergent += 1;
      }
    }
    return {
      total: results.length,
      verified,
      divergent,
      byPair,
      recent: results.slice(-20),
    };
  }
}
