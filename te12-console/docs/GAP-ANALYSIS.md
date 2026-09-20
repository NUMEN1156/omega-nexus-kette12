# TE12 Gap-Analyse — Soll (TE12/NeuroLumen-Doku) vs. Ist (Repos)

Stand: 2026-09-20. Verglichen wurden die TE12-Zieldokumentation mit
`NUMEN1156/omega-nexus-kette12` (KETTE12-Relay-Backbone) und
`NUMEN1156/dvp-settlement-prototype` (atomare DvP-Abwicklung, PoSoF-ZK).

Legende: **VORHANDEN** · **TEILWEISE** · **FEHLT** · **WIDERSPRUCH**

## 1. Präsentations-Schicht

| Anforderung TE12 | omega-nexus-kette12 | dvp-settlement-prototype | Status |
|---|---|---|---|
| Dark-Industrial-UI (`#0A0A0A` / `#1E1E1E` / `#00FF41`), Terminal-Typografie | `dashboard/index.html`: dunkles Theme (`#080b12`, Akzent `#43d9d0`), system-ui-Font | kein Frontend | TEILWEISE — Farbschema und Typografie weichen ab |
| Echtzeit-Pulse-Widgets | HTTP-Polling alle 5 s auf `:9090`-Metrics-JSON, kein Push | — | TEILWEISE — kein Push-Kanal (SSE/WebSocket) |
| Reaktives Dashboard mit Entropie-Trajektorien | nur Relay-Zähler (Sessions, Payloads, Timeouts) | — | FEHLT |

## 2. Logik-Schicht

| Anforderung TE12 | omega-nexus-kette12 | dvp-settlement-prototype | Status |
|---|---|---|---|
| PredictiveTensorController | `Core/` (Python Rekalibrierung/Accelerator), `brainfish-swarm/` (Graph/RAG) — kein Tensor-Controller, keine deterministische Trajektorie | — | FEHLT |
| KETTE12-Isolationsmuster | mTLS-Zertifikat CN/SAN = `node_id`, Node→Topic-ACL (`acl.rs`), Skill-Allow-Lists (`skills.rs`) | `docs/security/KETTE12-QUORUM-SPEC.md`: 12 Guardians, 8/12-Quorum — nur Spezifikation, kein Code | TEILWEISE — Node-Isolation ja, Quorum-Registry fehlt |
| Tenant-Cross-Validation | keine Tenant-IDs; Metrics prozessweit, Outbox geteilt | kein Tenant-Modell (globale Mappings, ein Admin) | FEHLT |
| Autonome Analysen | Brainfish/Accelerator liefern Analyse-Outputs, aber ohne Steuer-/Freigabe-Schleife | — | TEILWEISE |

## 3. Infrastruktur-Schicht

| Anforderung TE12 | omega-nexus-kette12 | dvp-settlement-prototype | Status |
|---|---|---|---|
| WORM-Master-Storage | Spezifiziert (`vault_manifest.json`, `blake3_root.json`, `ledger.jsonl`, `chmod 444`; CI-versiegelte SHA-256-Manifeste), zur Laufzeit **nicht** implementiert; SQLite-Outbox ist mutierbar | kein Laufzeit-Log; On-Chain-Events (`TradeInitiated/Settled/Cancelled`) sind de facto unveränderlich | TEILWEISE |
| SHA3-256-Verifizierung jedes Statuswechsels | Keccak-256 (Ethereum-Variante, explizit ≠ SHA3), SHA-256 in CI; BLAKE3/Ed25519 nur spezifiziert | keccak256, Poseidon (BN254), SHA-256 (JCS/TLV-Evidence), BLAKE2b-512 (Ceremony) | **WIDERSPRUCH** — SHA3-256 (FIPS 202) wird in keinem Repo verwendet |
| Immutable Logs / Audit-Kette | `tracing`-Logs, SQLite `outbox_events` mit Retry-Status — kein Hash-Chaining | RFC-8785-JCS + TLV-Frame, `sha256(frame)` als Evidence-Digest — Einzel-Digest, keine Kette | TEILWEISE |
| High-Concurrency Node-Sync | Tokio-Relay mit mTLS-Sessions, Topic-Routing, durable Outbox → `OutboxTimelock.sol` | — | VORHANDEN (Relay-Ebene) |

## 4. Funktionale Module

| Modul | Ist | Status |
|---|---|---|
| Reaktives Dashboard & Pulse-Widgets | Relay-Observability nur | TEILWEISE |
| Tensor-Labor | nichts Vergleichbares; PoSoF-Proof-Generierung/JCS-Fixtures könnten als „Labor“-Eingaben dienen | FEHLT |
| Auditor-Modul (KETTE12) | Compliance-Gate in CI (`compliance-gate.yml`: SLSA, OIDC, Rekor, SBOM); OpenAPI `architecture-scaffolding/openapi.yaml` mit `/core/*`, `/verification/*`, `/audit/*` — **kein** Server dahinter | TEILWEISE |
| Kryptografische WORM-Sicherung | siehe Infrastruktur | TEILWEISE |

## 5. Compliance & Standards

| Anspruch | Befund | Status |
|---|---|---|
| SEC 17 CFR § 240.17a-4 | in keinem Repo erwähnt oder adressiert | FEHLT |
| SOC 2 Type II | in keinem Repo erwähnt; keine Kontrollmatrix | FEHLT |
| ISO 27001 | `compliance/AUDIT_READINESS_v0.3.2.md` (dvp) referenziert ISO/IEC 27001, BSI TR-03109, NIS-2, EU AI Act — ausdrücklich **Pre-Audit-Selbsteinschätzung**, keine Zertifizierung | TEILWEISE |

**Wichtig:** Die TE12-Doku formuliert „erfüllt“ (SEC 17a-4, SOC 2 Type II, ISO 27001). Beide Repos sind laut eigenem README nicht produktiv und nicht auditiert. Diese Aussage muss in der TE12-Doku auf „ausgelegt für / vorbereitet auf“ abgeschwächt werden, bis externe Nachweise vorliegen (siehe Architektur-Doku §7).

## 6. Priorisierte Maßnahmen

1. **SHA3-256 festlegen und implementieren** (P0) — Alle bestehenden Digests sind Keccak-256/SHA-256/Poseidon. Entweder TE12-Doku auf die realen Algorithmen korrigieren oder eine SHA3-256-Audit-Schicht einführen, die bestehende Digests als Payload einbettet (so umgesetzt im Prototyp `te12-console/server/worm.js`).
2. **WORM-Laufzeit-Log mit Hash-Chaining** (P0) — Append-only JSONL, `hash_n = SHA3-256(hash_{n-1} ‖ canonical(entry_n))`, Verify-Endpunkt; anschließend periodische Versiegelung des Head-Hashes via `OutboxTimelock.queue(bytes)` auf L1 (Anker) und Rekor-Eintrag (bereits in CI vorhanden).
3. **Tenant-Modell** (P1) — Tenant-ID in Relay-Frames (`RelayMessage::Payload`) und Outbox-Zeilen; Cross-Validation = zwei unabhängige Tenants liefern denselben Digest für dasselbe Subjekt (Prototyp: `server/auditor.js`).
4. **Push-Kanal** (P1) — SSE oder WebSocket statt 5-s-Polling; Prototyp nutzt SSE (`/api/events`).
5. **KETTE12-Quorum-Registry** (P1) — `KETTE12NodeRegistry`/`KETTE12Relayer` aus der Spezifikation in `l1-contracts/` implementieren; Auditor-Modul liest Quorum-Zustand.
6. **API-Server hinter `openapi.yaml`** (P2) — `/audit/*` durch den TE12-Auditor bedienen, `/verification/*` an PoSoF-Verifier (dvp) delegieren.
7. **PredictiveTensorController produktiv** (P2) — Brainfish-/Accelerator-Outputs auf Tensor-Snapshots abbilden; Determinismus per Seed und Digest garantieren (Prototyp: `server/tensor.js`).
8. **Compliance-Nachweise** (P2) — 17a-4-Kontrollmatrix (Nicht-Überschreibbarkeit, Dauer, Zugriff, Audit-Trail, D3P), SOC-2-Kontrollkatalog; Wortlaut in TE12-Doku bis dahin korrigieren.

## 7. Integrationspunkte für die TE12-Konsole

- **omega-nexus-kette12:** Metrics-JSON `:9090` (Dashboard), mTLS-Relay-Topics (Tensor-/Auditor-Jobs), SQLite-Outbox → `OutboxTimelock` (Verankerung des WORM-Heads), `scripts/ws-event-listener.mjs` (`eth_subscribe` für `Queued/Executed/Cancelled`).
- **dvp-settlement-prototype:** Contract-Events `TradeInitiated/TradeSettled/TradeCancelled`, Nullifier-/Root-Events, JCS-Evidence-Digest `sha256(frame)` als Cross-Validation-Subjekt.
