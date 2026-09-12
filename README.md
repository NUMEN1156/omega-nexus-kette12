# omega-nexus-kette12

Kette12 Relay backbone for the omega-nexus architecture.

## Modules

- `arche-omega-relayer` — async TCP relay core (Tokio)
- `l1-contracts` — Foundry project; `OutboxTimelock` is the L1 landing contract for outbox events (submitter-only `queue(bytes)`, timelocked `execute`, guardian `cancel`)
- `scripts/anvil-e2e.sh` — host-only Anvil E2E pipeline; `scripts/ws-event-listener.mjs` — WebSocket `eth_subscribe` log catcher

## Anvil E2E pipeline

```bash
./scripts/anvil-e2e.sh   # requires anvil/forge/cast, cargo, node >= 22, python3, openssl
```

Local Node -> Deploy `OutboxTimelock` -> build + start relayer with `L1_EVM_ENABLED=true` -> mTLS publish on `clap.embedding.request` -> `EvmSink` submits the payload as an ABI-encoded `queue(bytes)` call -> `Queued` caught over WebSocket -> `execute()` before `eta` fails closed (`TimelockNotElapsed`) -> `evm_increaseTime` + `evm_mine` -> `execute()` -> `Executed` caught -> replay rejected. Logs land in `.e2e-anvil/`.

Relay ports can be moved with `RELAY_BIND_ADDR` / `RELAY_HEALTH_ADDR`.

## Roadmaps

- `ARCHE-GOLD-PROTOCOL-PHASE-II-ROADMAP.md` — Phase-II governance, quarterly decision gates, and scaling targets (Q3 2026–Q2 2027)

## Option B skill layer

The relay now exposes a fail-closed skill transport for the first two KETTE12 scouts:

- `skill.browser.request` / `skill.browser.result` — Browser Use for the `outreach` scout
- `skill.scientific.request` / `skill.scientific.result` — Scientific analysis for the `label` scout

The relay remains responsible only for transport, ACL enforcement, durable outbox handling, and observability. Skill execution stays outside the relay core.

### Fail-closed request contracts

Skill topics are validated as JSON before routing:

- Browser requests must include `scout`, `task_id`, `target`, `allowed_targets`, and `output_topic`
- Scientific requests must include `scout`, `task_id`, `source`, `query`, `allowed_sources`, and `output_topic`
- Requests without an explicit allow-list policy are rejected
- Browser requests are currently restricted to the `outreach` scout
- Scientific requests are currently restricted to the `label` scout
- Result payloads must include `scout`, `task_id`, `status`, and `summary`

Successful results must include at least one artifact. Non-success results must include an explicit error field so timeouts and denials remain visible to operators.

### Topic ACL example

Set `RELAY_ACL` so request publishers and result consumers are explicitly authorized:

```text
outreach-scout=skill.browser.request|skill.browser.result,label-scout=skill.scientific.request|skill.scientific.result,browser-worker=skill.browser.request|skill.browser.result,scientific-worker=skill.scientific.request|skill.scientific.result
```

### Observability

The health endpoint and dashboard now expose:

- total skill requests/results
- browser vs scientific request counts
- success/failure/timeout/denied/rejected counts for the skill layer
