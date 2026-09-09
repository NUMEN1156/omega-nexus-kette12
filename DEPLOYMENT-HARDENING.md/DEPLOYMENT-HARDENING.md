# Omega Nexus Deployment Hardening



This package prepares the target host without starting containers or enabling settlement.



## Secrets



Copy `.env.stack.example` to `.env.stack` on the host and use mode 600. Never commit `.env.stack`, private keys, RPC credentials, or model artifacts. Prefer Docker Secrets or a host secret manager for production.



The example keeps `L1_EVM_ENABLED=false`. Enabling settlement requires explicit operator approval, valid sender and destination addresses, chain-ID verification, and an RPC node whose sender authorization has been reviewed.



## mTLS



Provision `ca.crt`, `server.crt`, and `server.key` under `arche-omega-relayer/certs/`. Keep `server.key` owner-readable only. Client certificates must be issued by the configured CA; do not weaken certificate verification to recover from a failed handshake.



## Preflight and deployment



```bash

cp .env.stack.example .env.stack

chmod 600 .env.stack arche-omega-relayer/certs/server.key

bash scripts/preflight.sh

bash scripts/deploy.sh

```



The preflight is read-only. It checks Docker, Compose rendering, env permissions, mTLS files, fail-closed EVM configuration, and optionally `eth_chainId` with `--require-rpc`. The deployment script waits for relay health and never prints secret values.



## Network exposure



Keep relay, health, dashboard, and RPC ports bound to localhost or an authenticated reverse proxy. Do not publicly bind port 8545. Apply firewall policy before exposing any operational endpoint.



## Activation gates



1. Validate certificates and relay health with settlement disabled.

2. Run Anvil E2E and outbox restart/recovery tests.

3. Test RPC outage recovery without deleting persistent data.

4. Record chain ID, RPC ownership, sender, destination, payload policy, backup point, and incident owner.

5. Run `bash scripts/preflight.sh --require-rpc`.

6. Use `bash scripts/deploy.sh --allow-live-evm` only after the human approval record exists.



The dashboard is read-only. Preserve logs, transaction hashes, effective non-secret Compose configuration, and the outbox database during incidents. Never use `docker compose down --volumes` for routine shutdown or triage.

