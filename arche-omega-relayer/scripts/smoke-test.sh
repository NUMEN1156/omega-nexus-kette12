#!/usr/bin/env bash
# Lightweight smoke test against a running relay (health + basic expectations).
set -euo pipefail

HEALTH_URL="${HEALTH_URL:-http://127.0.0.1:9090/}"
RELAY_HOST="${RELAY_HOST:-127.0.0.1}"
RELAY_PORT="${RELAY_PORT:-8080}"
MAX_WAIT="${MAX_WAIT:-60}"

echo "==> Waiting for health endpoint at $HEALTH_URL (max ${MAX_WAIT}s)..."
for i in $(seq 1 "$MAX_WAIT"); do
  if curl -sf "$HEALTH_URL" >/dev/null 2>&1; then
    echo "    health is up after ${i}s"
    break
  fi
  if [ "$i" -eq "$MAX_WAIT" ]; then
    echo "ERROR: health endpoint did not become ready" >&2
    exit 1
  fi
  sleep 1
done

echo "==> Fetching metrics snapshot..."
SNAPSHOT=$(curl -sf "$HEALTH_URL")
echo "$SNAPSHOT" | head -c 500
echo

# Basic JSON shape checks (no jq required)
if ! echo "$SNAPSHOT" | grep -q '"status":"ok"'; then
  echo "ERROR: expected status=ok" >&2
  exit 1
fi
if ! echo "$SNAPSHOT" | grep -q '"active_sessions"'; then
  echo "ERROR: missing active_sessions field" >&2
  exit 1
fi
if ! echo "$SNAPSHOT" | grep -q '"total_handshakes"'; then
  echo "ERROR: missing total_handshakes field" >&2
  exit 1
fi

echo "==> Checking relay TCP port ${RELAY_HOST}:${RELAY_PORT}..."
if command -v nc >/dev/null 2>&1; then
  if ! nc -z "$RELAY_HOST" "$RELAY_PORT" 2>/dev/null; then
    echo "ERROR: relay port not accepting connections" >&2
    exit 1
  fi
  echo "    TCP port is open"
else
  echo "    (nc not available, skipping TCP probe)"
fi

echo "==> Smoke test PASSED"
