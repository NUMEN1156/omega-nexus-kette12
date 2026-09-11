# omega-nexus-kette12

Kette12 Relay backbone for the omega-nexus architecture.

## Modules

- `arche-omega-relayer` — async TCP relay core (Tokio)

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
