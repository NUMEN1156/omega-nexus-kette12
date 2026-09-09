# Omega Nexus Operations Runbook



This runbook is for the target host. Settlement is disabled by default and all commands avoid printing secret values.



## Dry-run deployment



```bash

cp .env.stack.example .env.stack

chmod 600 .env.stack arche-omega-relayer/certs/server.key

bash scripts/preflight.sh

bash scripts/deploy.sh

```



For the Anvil simulation profile:



```bash

bash scripts/preflight.sh

bash scripts/deploy.sh --profile e2e

```



Readiness requires Compose validation, relay health (`status: ok`), valid mTLS material, a persistent outbox file, and a dry-run sink. The dashboard is read-only and must remain behind an authenticated TLS boundary if exposed beyond localhost.



## Outbox restart recovery



Never delete the persistent outbox database during a restart test. Stop and start only the relay, then rerun the deployment health gate:



```bash

docker compose --env-file .env.stack -f docker-compose.yml stop relay

docker compose --env-file .env.stack -f docker-compose.yml start relay

bash scripts/deploy.sh

```



Verify that pending event IDs survive, retries remain possible, and published events are not duplicated beyond the sink idempotency policy. Back up `.e2e-data/outbox.sqlite3` while the relay is stopped before destructive cleanup.



## RPC outage test



Run only with `L1_EVM_ENABLED=false` or against Anvil. Stop the RPC service, create one controlled event, verify that the event remains recoverable, restart RPC, and reconcile the resulting transaction hash with the outbox event ID. A failed RPC call is not proof that a transaction was not mined.



## Live EVM gate



Live settlement is a separate human-approved operation. The deployment script refuses `L1_EVM_ENABLED=true` unless `--allow-live-evm` is supplied. Before using it, record chain ID, RPC ownership, sender and destination, payload policy, backup point, and incident owner. Then run:



```bash

bash scripts/preflight.sh --require-rpc

bash scripts/deploy.sh --allow-live-evm

```



Start with one controlled event and verify receipt status, chain ID, transaction hash, and outbox state before increasing volume.



## Incident response and stop



For unexpected transactions, certificate failures, RPC errors, or outbox growth, set `L1_EVM_ENABLED=false`, preserve logs and the outbox, and recreate only the affected service. Do not wipe volumes during triage. Routine shutdown must not include `--volumes`:



```bash

docker compose --env-file .env.stack -f docker-compose.yml stop relay clap-provider test-orchestrator

```

