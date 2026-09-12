#!/usr/bin/env bash
# Anvil E2E pipeline: Local Node -> Deploy -> Relay publish -> Timelock Advance -> Event Catch
#
# Runs entirely on the host (no Docker):
#   1. start Anvil
#   2. deploy l1-contracts/OutboxTimelock
#   3. build + start arche-omega-relayer with the EVM sink pointed at the contract
#   4. start the WebSocket log listener (scripts/ws-event-listener.mjs)
#   5. publish an outbox payload through the mTLS relay -> EvmSink -> queue(bytes) -> Queued
#   6. assert execute() fails closed before eta, advance chain time, execute() -> Executed
#   7. assert the listener saw both events and the outbox drained
#
# Env overrides: ANVIL_PORT, RELAY_PORT, HEALTH_PORT, TIMELOCK_DELAY, E2E_DIR, E2E_TIMEOUT
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANVIL_PORT="${ANVIL_PORT:-8545}"
RELAY_PORT="${RELAY_PORT:-8080}"
HEALTH_PORT="${HEALTH_PORT:-9090}"
TIMELOCK_DELAY="${TIMELOCK_DELAY:-3600}"
E2E_DIR="${E2E_DIR:-$ROOT/.e2e-anvil}"
E2E_TIMEOUT="${E2E_TIMEOUT:-90}"
CHAIN_ID=31337
RPC_URL="http://127.0.0.1:${ANVIL_PORT}"
WS_URL="ws://127.0.0.1:${ANVIL_PORT}"
TOPIC="clap.embedding.request"
PAYLOAD_TEXT="${PAYLOAD_TEXT:-{\"topic\":\"${TOPIC}\",\"kette\":12,\"run\":$(date +%s)}}"

# Anvil default account #0 (publicly known test key)
DEPLOYER_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
DEPLOYER="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
GUARDIAN="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

PIDS=()
log()  { printf '\033[1;34m[e2e]\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m[e2e] FAIL:\033[0m %s\n' "$*" >&2; exit 1; }
cleanup() {
  for pid in "${PIDS[@]:-}"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
}
trap cleanup EXIT

wait_for() { # wait_for <label> <seconds> <cmd...>
  local label="$1" secs="$2"; shift 2
  for _ in $(seq 1 "$secs"); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  fail "timeout waiting for ${label}"
}

for tool in anvil forge cast node python3 openssl cargo curl; do
  command -v "$tool" >/dev/null || fail "missing required tool: $tool"
done
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 22 ] || fail "node >= 22 required for the built-in WebSocket client (found $(node -v))"

rm -rf "$E2E_DIR"
mkdir -p "$E2E_DIR/data" "$E2E_DIR/certs"
cd "$ROOT"

# ---------------------------------------------------------------- 1. Anvil
log "starting anvil on :${ANVIL_PORT}"
anvil --host 127.0.0.1 --port "$ANVIL_PORT" --chain-id "$CHAIN_ID" --silent >"$E2E_DIR/anvil.log" 2>&1 &
PIDS+=($!)
wait_for "anvil rpc" 30 cast chain-id --rpc-url "$RPC_URL"

# ---------------------------------------------------------------- 2. Deploy
log "building + deploying OutboxTimelock(delay=${TIMELOCK_DELAY}, submitter=${DEPLOYER}, guardian=${GUARDIAN})"
(cd l1-contracts && forge build --silent)
DEPLOY_JSON="$(cd l1-contracts && forge create src/OutboxTimelock.sol:OutboxTimelock \
  --rpc-url "$RPC_URL" --private-key "$DEPLOYER_KEY" --broadcast --json \
  --constructor-args "$TIMELOCK_DELAY" "$DEPLOYER" "$GUARDIAN")"
CONTRACT="$(printf '%s' "$DEPLOY_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin)["deployedTo"])')"
[[ "$CONTRACT" =~ ^0x[0-9a-fA-F]{40}$ ]] || fail "could not parse deployed address from: $DEPLOY_JSON"
log "OutboxTimelock deployed at ${CONTRACT}"
[ "$(cast call "$CONTRACT" 'delay()(uint256)' --rpc-url "$RPC_URL")" = "$TIMELOCK_DELAY" ] || fail "delay() mismatch"

# ---------------------------------------------------------------- 3. Relayer
log "building arche-omega-relayer"
cargo build --release -p arche-omega-relayer --quiet
RELAYER_BIN="$ROOT/target/release/arche-omega-relayer"
[ -x "$RELAYER_BIN" ] || fail "relayer binary not found at $RELAYER_BIN"

log "generating mTLS test certificates"
bash arche-omega-relayer/scripts/gen-test-certs.sh "$E2E_DIR/certs" >/dev/null

log "starting relayer (EVM sink -> ${CONTRACT})"
(
  cd "$E2E_DIR"
  RUST_LOG=info \
  RELAY_ACL="clap-provider-1=${TOPIC}" \
  OUTBOX_TOPIC_FILTERS="$TOPIC" \
  L1_EVM_ENABLED=true \
  L1_EVM_RPC_URL="$RPC_URL" \
  L1_EVM_FROM="$DEPLOYER" \
  L1_EVM_TO="$CONTRACT" \
  L1_EVM_CHAIN_ID="$CHAIN_ID" \
  RELAY_BIND_ADDR="127.0.0.1:${RELAY_PORT}" \
  RELAY_HEALTH_ADDR="127.0.0.1:${HEALTH_PORT}" \
  exec "$RELAYER_BIN" >"$E2E_DIR/relayer.log" 2>&1
) &
PIDS+=($!)
wait_for "relayer health" 30 curl -sf "http://127.0.0.1:${HEALTH_PORT}/"

# ---------------------------------------------------------------- 4. Listener
log "starting WebSocket event listener on ${WS_URL}"
node scripts/ws-event-listener.mjs --ws "$WS_URL" --address "$CONTRACT" \
  --expect Queued,Executed --timeout "$E2E_TIMEOUT" --ready-file "$E2E_DIR/listener.ready" \
  >"$E2E_DIR/listener.log" 2>&1 &
LISTENER_PID=$!
PIDS+=($LISTENER_PID)
wait_for "listener subscription" 15 test -s "$E2E_DIR/listener.ready"

# ---------------------------------------------------------------- 5. Publish via relay
log "publishing payload through mTLS relay on topic ${TOPIC}"
PAYLOAD_HEX="0x$(printf '%s' "$PAYLOAD_TEXT" | od -An -tx1 | tr -d ' \n')"
PAYLOAD_HASH="$(cast keccak "$PAYLOAD_HEX")"
ACK="$(python3 - "$E2E_DIR/certs" "$RELAY_PORT" "$TOPIC" "$PAYLOAD_TEXT" <<'PY'
import pathlib, sys
sys.path.insert(0, "arche-omega-relayer/tests")
from e2e_orchestrator import publish
cert_dir, port, topic, payload = pathlib.Path(sys.argv[1]), int(sys.argv[2]), sys.argv[3], sys.argv[4].encode()
print(publish(cert_dir, "127.0.0.1", port, topic, payload))
PY
)"
[[ "$ACK" == published:* ]] || fail "relay did not ack payload: $ACK"
log "relay ack: ${ACK}"

log "waiting for Queued event via WebSocket"
wait_for "Queued event" "$E2E_TIMEOUT" grep -q '"name":"Queued"' "$E2E_DIR/listener.log"
QUEUED_LINE="$(grep '"name":"Queued"' "$E2E_DIR/listener.log" | head -1)"
ITEM_ID="$(printf '%s' "$QUEUED_LINE" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
SEEN_HASH="$(printf '%s' "$QUEUED_LINE" | python3 -c 'import json,sys; print(json.load(sys.stdin)["payloadHash"])')"
[ "$SEEN_HASH" = "$PAYLOAD_HASH" ] || fail "payload hash mismatch: on-chain ${SEEN_HASH} vs local ${PAYLOAD_HASH}"
log "Queued id=${ITEM_ID} payloadHash=${PAYLOAD_HASH}"

wait_for "outbox drain" 15 bash -c "python3 -c \"import sqlite3;db=sqlite3.connect('$E2E_DIR/data/outbox.sqlite3');p,q=db.execute('select sum(published_at is null),sum(published_at is not null) from outbox_events').fetchone();exit(0 if (p==0 and q==1) else 1)\""
log "outbox drained: 0 pending, 1 published"

# ---------------------------------------------------------------- 6. Timelock
EXECUTE_CALLDATA="$(cast calldata 'execute(uint256,bytes)' "$ITEM_ID" "$PAYLOAD_HEX")"
if cast call "$CONTRACT" "$EXECUTE_CALLDATA" --rpc-url "$RPC_URL" --from "$DEPLOYER" >"$E2E_DIR/early-execute.log" 2>&1; then
  fail "execute() succeeded before the timelock elapsed"
fi
EXPECTED_SELECTOR="$(cast sig 'TimelockNotElapsed(uint256,uint256,uint256)')"
grep -qi "${EXPECTED_SELECTOR#0x}" "$E2E_DIR/early-execute.log" || fail "early execute did not revert with TimelockNotElapsed: $(cat "$E2E_DIR/early-execute.log")"
log "execute() before eta fails closed with TimelockNotElapsed"

log "advancing chain time by ${TIMELOCK_DELAY}s"
cast rpc evm_increaseTime "$TIMELOCK_DELAY" --rpc-url "$RPC_URL" >/dev/null
cast rpc evm_mine --rpc-url "$RPC_URL" >/dev/null
[ "$(cast call "$CONTRACT" 'isReady(uint256)(bool)' "$ITEM_ID" --rpc-url "$RPC_URL")" = "true" ] || fail "item not ready after time advance"

log "executing queued item ${ITEM_ID}"
cast send "$CONTRACT" 'execute(uint256,bytes)' "$ITEM_ID" "$PAYLOAD_HEX" \
  --rpc-url "$RPC_URL" --private-key "$DEPLOYER_KEY" --json >"$E2E_DIR/execute-tx.json"
[ "$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["status"])' "$E2E_DIR/execute-tx.json")" = "0x1" ] || fail "execute tx reverted"

# ---------------------------------------------------------------- 7. Event catch
log "waiting for listener to observe Executed"
if wait "$LISTENER_PID"; then
  :
else
  fail "listener exited non-zero: $(tail -3 "$E2E_DIR/listener.log")"
fi
grep -q '"name":"Executed"' "$E2E_DIR/listener.log" || fail "Executed event not observed"
EXECUTED_HASH="$(grep '"name":"Executed"' "$E2E_DIR/listener.log" | head -1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["payloadHash"])')"
[ "$EXECUTED_HASH" = "$PAYLOAD_HASH" ] || fail "Executed payloadHash mismatch"

if cast call "$CONTRACT" "$EXECUTE_CALLDATA" --rpc-url "$RPC_URL" --from "$DEPLOYER" >/dev/null 2>&1; then
  fail "replaying execute() did not revert"
fi

log "listener events:"
grep '"level":"event"' "$E2E_DIR/listener.log" | sed 's/^/       /'
log "E2E PASS (contract=${CONTRACT} item=${ITEM_ID} logs in ${E2E_DIR})"
