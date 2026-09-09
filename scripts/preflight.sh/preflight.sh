#!/usr/bin/env bash

set -Eeuo pipefail



# Read-only deployment preflight. It never prints secret values and never mutates Docker state.

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env.stack}"

COMPOSE_FILE="${COMPOSE_FILE:-${ROOT_DIR}/docker-compose.yml}"

CERT_DIR="${CERT_DIR:-${ROOT_DIR}/arche-omega-relayer/certs}"

require_rpc="false"

usage() {

  cat <<'EOF'

Usage: preflight.sh [--require-rpc]



Environment overrides: ENV_FILE, COMPOSE_FILE, CERT_DIR

--require-rpc  require a successful JSON-RPC probe when L1_EVM_ENABLED=true

EOF

}

while (($#)); do

  case "$1" in

    --require-rpc) require_rpc="true" ;;

    -h|--help) usage; exit 0 ;;

    *) echo "ERROR: unknown argument: $1" >&2; usage >&2; exit 2 ;;

  esac

  shift

done

fail() { echo "PREFLIGHT_FAIL: $*" >&2; exit 1; }

ok() { echo "PREFLIGHT_OK: $*"; }

command -v docker >/dev/null 2>&1 || fail "docker is not installed or not on PATH"

docker info >/dev/null 2>&1 || fail "Docker daemon is unavailable"

docker compose version >/dev/null 2>&1 || fail "Docker Compose v2 is unavailable"

[[ -f "$COMPOSE_FILE" ]] || fail "compose file not found: $COMPOSE_FILE"

[[ -f "$ENV_FILE" ]] || fail "environment file not found: $ENV_FILE"

mode="$(stat -c '%a' "$ENV_FILE")"

(( 10#$mode <= 600 )) || fail "$ENV_FILE permissions are $mode; require 600 or stricter"

ok "Docker and Compose available; env file permissions are $mode"

get_env() {

  local key="$1" value=""

  value="$(awk -F= -v k="$key" '$1 == k {sub(/^[^=]*=/, ""); print; exit}' "$ENV_FILE")"

  printf '%s' "${!key:-$value}"

}

L1_EVM_ENABLED="$(get_env L1_EVM_ENABLED | tr '[:upper:]' '[:lower:]')"

L1_EVM_RPC_URL="$(get_env L1_EVM_RPC_URL)"

L1_EVM_FROM="$(get_env L1_EVM_FROM)"

L1_EVM_TO="$(get_env L1_EVM_TO)"

case "$L1_EVM_ENABLED" in

  ""|false|0|no|off) ok "EVM sink remains disabled/dry-run" ;;

  true|1|yes|on)

    [[ -n "$L1_EVM_RPC_URL" ]] || fail "L1_EVM_RPC_URL is required when EVM sink is enabled"

    [[ "$L1_EVM_FROM" =~ ^0x[0-9a-fA-F]{40}$ ]] || fail "L1_EVM_FROM is not a valid 20-byte address"

    [[ "$L1_EVM_TO" =~ ^0x[0-9a-fA-F]{40}$ ]] || fail "L1_EVM_TO is not a valid 20-byte address"

    ok "EVM sink configuration has non-secret address shape and RPC URL"

    if [[ "$require_rpc" == true ]]; then

      rpc_body='{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'

      rpc_response="$(curl --fail-with-body --silent --show-error --max-time 5 -H 'content-type: application/json' --data "$rpc_body" "$L1_EVM_RPC_URL")" || fail "EVM JSON-RPC probe failed"

      grep -q '"result"' <<<"$rpc_response" || fail "EVM JSON-RPC response did not contain result"

      ok "EVM JSON-RPC probe succeeded"

    fi

    ;;

  *) fail "L1_EVM_ENABLED must be false or true" ;;

esac

[[ -d "$CERT_DIR" ]] || fail "certificate directory not found: $CERT_DIR"

for required in ca.crt server.crt server.key; do

  [[ -s "$CERT_DIR/$required" ]] || fail "missing or empty certificate file: $CERT_DIR/$required"

done

[[ "$(stat -c '%a' "$CERT_DIR/server.key")" =~ ^6?00$|^640$ ]] || fail "server.key must be owner-readable only"

ok "required mTLS files present with restrictive private-key permissions"

docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" config --quiet || fail "docker compose config validation failed"

ok "Compose configuration renders successfully"

echo "PREFLIGHT_PASS: read-only checks completed; no containers were started or changed"

