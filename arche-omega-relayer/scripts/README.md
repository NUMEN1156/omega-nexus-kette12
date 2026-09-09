# Local integration / smoke test environment

## 1. Generate test certificates

```bash
cd arche-omega-relayer
chmod +x scripts/gen-test-certs.sh
./scripts/gen-test-certs.sh certs
```

Produces: `ca.crt`, `server.crt`, `server.key`, `client.crt`, `client.key`

## 2. Start the relay (Docker)

From the repository root:

```bash
docker compose up --build -d relay
```

- mTLS relay: `localhost:8080`
- Health/metrics: `http://localhost:9090/`

## 3. Run smoke test

```bash
chmod +x arche-omega-relayer/scripts/smoke-test.sh
./arche-omega-relayer/scripts/smoke-test.sh
```

## 4. Optional: start clap-provider against the relay

```bash
docker compose --profile provider up --build clap-provider
```

## 5. Tear down

```bash
docker compose down
```

## Notes

- Certificates are mounted read-only into the container at `/app/certs`.
- The health endpoint is plain HTTP (no TLS) on a separate port.
- The clap-provider binary expects client certs under the same `certs/` paths used by the relay config defaults (or via env — extend as needed).
