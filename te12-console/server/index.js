import { createReadStream, mkdirSync, statSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, relative, resolve } from "node:path";
import { Auditor } from "./auditor.js";
import { PredictiveTensorController } from "./tensor.js";
import { WormLog } from "./worm.js";

const PACKAGE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PUBLIC_ROOT = resolve(PACKAGE_ROOT, "public");
const CONTENT_TYPES = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
};
const MAX_BODY = 256 * 1024;

function sendJson(response, status, value) {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
  });
  response.end(body);
}

function parseBody(request) {
  return new Promise((resolveBody, reject) => {
    let size = 0;
    let text = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      size += Buffer.byteLength(chunk);
      if (size > MAX_BODY) {
        reject(new Error("request body too large"));
        request.resume();
        return;
      }
      text += chunk;
    });
    request.on("end", () => {
      if (size === 0) {
        resolveBody({});
        return;
      }
      try {
        resolveBody(JSON.parse(text));
      } catch {
        reject(new Error("invalid JSON"));
      }
    });
    request.on("error", reject);
  });
}

function safePublicPath(pathname) {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0") || decoded.includes("\\") || decoded.split("/").includes("..")) {
    return null;
  }
  const candidate = resolve(PUBLIC_ROOT, decoded === "/" ? "index.html" : decoded.slice(1));
  const rel = relative(PUBLIC_ROOT, candidate);
  if (rel.startsWith("..") || rel.split(/[\\/]/).includes("..")) {
    return null;
  }
  return candidate;
}

function statePayload({ startedAt, startedMs, tensor, worm, auditor }) {
  return {
    system: {
      name: "TE12",
      version: "0.1.0",
      startedAt,
      uptimeSec: Math.floor((Date.now() - startedMs) / 1000),
    },
    tensor: tensor.snapshot(),
    trajectory: tensor.trajectory.map((point) => ({ ...point })),
    worm: { length: worm.entries().length, headHash: worm.head() },
    auditor: auditor.matrix(),
  };
}

export function createServer({ dataDir } = {}) {
  const resolvedDataDir = dataDir
    ? resolve(dataDir)
    : process.env.DATA_DIR
      ? resolve(PACKAGE_ROOT, process.env.DATA_DIR)
      : resolve(PACKAGE_ROOT, "data");
  mkdirSync(resolvedDataDir, { recursive: true });
  const worm = new WormLog({ filePath: join(resolvedDataDir, "worm.jsonl") });
  const tensor = new PredictiveTensorController();
  const auditor = new Auditor({ worm });
  const startedAt = new Date().toISOString();
  const startedMs = Date.now();
  const clients = new Set();

  const broadcast = (event, value) => {
    const message = `event: ${event}\ndata: ${JSON.stringify(value)}\n\n`;
    for (const client of clients) {
      try {
        client.response.write(message);
      } catch {
        client.response.destroy();
      }
    }
  };
  worm.onAppend((entry) => broadcast("worm", entry));
  worm.append({
    tenant: "TE12-CORE",
    kind: "SYSTEM",
    payload: { action: "boot", pid: process.pid, version: "0.1.0" },
  });

  const server = createHttpServer(async (request, response) => {
    const url = new URL(request.url, "http://localhost");
    const { pathname } = url;
    try {
      if (pathname === "/api/events" && request.method === "GET") {
        response.writeHead(200, {
          "cache-control": "no-cache",
          connection: "keep-alive",
          "content-type": "text/event-stream; charset=utf-8",
        });
        const client = { response };
        clients.add(client);
        response.write(
          `event: state\ndata: ${JSON.stringify(
            statePayload({
              startedAt,
              startedMs,
              tensor,
              worm,
              auditor,
            }),
          )}\n\n`,
        );
        client.pulse = setInterval(() => {
          try {
            response.write(
              `event: pulse\ndata: ${JSON.stringify({
                ts: new Date().toISOString(),
                tick: tensor.tick,
                entropy: tensor.snapshot().entropy,
                wormLength: worm.entries().length,
                headHash: worm.head(),
                clients: clients.size,
              })}\n\n`,
            );
          } catch {
            response.destroy();
          }
        }, 1000);
        client.heartbeat = setInterval(() => response.write(": heartbeat\n\n"), 15000);
        request.on("close", () => {
          clearInterval(client.pulse);
          clearInterval(client.heartbeat);
          clients.delete(client);
        });
        return;
      }

      if (pathname.startsWith("/api/")) {
        if (request.method === "GET" && pathname === "/api/state") {
          sendJson(
            response,
            200,
            statePayload({
              startedAt,
              startedMs,
              tensor,
              worm,
              auditor,
            }),
          );
          return;
        }
        if (request.method === "GET" && pathname === "/api/auditor/matrix") {
          sendJson(response, 200, auditor.matrix());
          return;
        }
        if (request.method === "GET" && pathname === "/api/worm") {
          const limit = url.searchParams.get("limit");
          sendJson(
            response,
            200,
            worm.entries({
              tenant: url.searchParams.get("tenant") || undefined,
              limit: limit === null ? 100 : limit,
            }),
          );
          return;
        }
        if (request.method === "GET" && pathname === "/api/worm/verify") {
          const verify = worm.verify();
          const entry = worm.append({
            tenant: "AUDITOR",
            kind: "CHAIN_VERIFY",
            payload: verify,
          });
          sendJson(response, 200, { verify, entry });
          return;
        }
        if (request.method === "POST") {
          const body = await parseBody(request);
          if (pathname === "/api/tensor/step") {
            const snapshot = tensor.step();
            const entry = worm.append({
              tenant: body.tenant || "TE12-CORE",
              kind: "TENSOR_STEP",
              payload: snapshot,
            });
            sendJson(response, 200, { snapshot, entry });
            return;
          }
          if (pathname === "/api/tensor/apply") {
            const snapshot = tensor.apply(body.matrix);
            const entry = worm.append({
              tenant: body.tenant || "TE12-CORE",
              kind: "TENSOR_APPLY",
              payload: { matrix: body.matrix, snapshot },
            });
            sendJson(response, 200, { snapshot, entry });
            return;
          }
          if (pathname === "/api/tensor/reset") {
            const snapshot = tensor.reset(body.seed);
            const entry = worm.append({
              tenant: "TE12-CORE",
              kind: "SYSTEM",
              payload: { action: "reset", seed: body.seed },
            });
            sendJson(response, 200, { snapshot, entry });
            return;
          }
          if (pathname === "/api/auditor/cross-validate") {
            sendJson(response, 200, auditor.crossValidate(body));
            return;
          }
        }
        sendJson(response, 404, { error: "not found" });
        return;
      }

      if (request.method !== "GET" && request.method !== "HEAD") {
        sendJson(response, 404, { error: "not found" });
        return;
      }
      const filePath = safePublicPath(pathname);
      if (!filePath) {
        sendJson(response, 404, { error: "not found" });
        return;
      }
      let stats;
      try {
        stats = statSync(filePath);
      } catch {
        sendJson(response, 404, { error: "not found" });
        return;
      }
      if (!stats.isFile()) {
        sendJson(response, 404, { error: "not found" });
        return;
      }
      response.writeHead(200, {
        "content-type": CONTENT_TYPES[extname(filePath)] || "application/octet-stream",
      });
      if (request.method === "HEAD") {
        response.end();
      } else {
        createReadStream(filePath).pipe(response);
      }
    } catch (error) {
      const status =
        request.method === "POST" ||
        error.message === "invalid JSON" ||
        error.message === "request body too large"
          ? 400
          : 500;
      sendJson(response, status, { error: error.message });
    }
  });

  return server;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const host = process.env.HOST || "127.0.0.1";
  const port = Number(process.env.PORT || 8712);
  const server = createServer();
  server.listen(port, host, () => {
    console.log(`TE12 console listening on http://${host}:${port}`);
  });
}
