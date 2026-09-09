#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env.stack}"

COMPOSE_FILE="${COMPOSE_FILE:-${ROOT_DIR}/docker-compose.yml}"

HEALTH_URL="${HEALTH_URL:-http://127.0.0.1:9090/}"

WAIT_SECONDS=90

PROFILE=""

ALLOW_LIVE_EVM=false

usage(){ echo "Usage: bash scripts/deploy.sh [--profile NAME] [--health-url URL] [--wait-seconds N] [--allow-live-evm]"; }

while (($#)); do case "$1" in --profile) PROFILE="$2"; shift 2;; --health-url) HEALTH_URL="$2"; shift 2;; --wait-seconds) WAIT_SECONDS="$2"; shift 2;; --allow-live-evm) ALLOW_LIVE_EVM=true; shift;; -h|--help) usage; exit 0;; *) echo "unknown argument: $1" >&2; usage >&2; exit 2;; esac; done

fail(){ echo "DEPLOY_FAIL: $*" >&2; exit 1; }

[[ -f "$ENV_FILE" ]] || fail "environment file not found"

EVM_ENABLED="$(awk -F= '$1 == "L1_EVM_ENABLED" {print tolower($2); exit}' "$ENV_FILE" | tr -d '[:space:]')"

if [[ "$EVM_ENABLED" =~ ^(true|1|yes|on)$ && "$ALLOW_LIVE_EVM" != true ]]; then fail "L1_EVM_ENABLED=true requires explicit --allow-live-evm approval"; fi

if [[ "$EVM_ENABLED" =~ ^(true|1|yes|on)$ ]]; then bash "$ROOT_DIR/scripts/preflight.sh" --require-rpc; else bash "$ROOT_DIR/scripts/preflight.sh"; fi

compose=(docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE")

[[ -n "$PROFILE" ]] && compose+=(--profile "$PROFILE")

"${compose[@]}" up -d --build relay

end=$((SECONDS + WAIT_SECONDS))

while ((SECONDS < end)); do

  if response="$(curl --silent --show-error --fail --max-time 3 "$HEALTH_URL" 2>/dev/null)" && grep -q '"status"[[:space:]]*:[[:space:]]*"ok"' <<<"$response"; then

    echo "DEPLOY_PASS: relay health endpoint is operational"; "${compose[@]}" ps; exit 0

  fi

  sleep 3

done

"${compose[@]}" ps >&2 || true

"${compose[@]}" logs --no-color --tail=80 relay >&2 || true

fail "relay health endpoint did not become operational within ${WAIT_SECONDS}s"

