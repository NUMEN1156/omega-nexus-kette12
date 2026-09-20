# TE12 Industrial Architecture — NeuroLumen Logic Core

Version 0.1.0 · Referenzimplementierung: `te12-console/`

## 1. Systemprojektion & Zweck

TE12 ist die industrielle Steuerkonsole des KETTE12-Backbones. Sie macht
deterministische Vektorräume (Tensor-Zustände, Entropie-Trajektorien) sichtbar
und dient als erweiterter, revisionssicherer Arbeitsspeicher für nicht-lineare
Prozesse: Jeder Zustandswechsel wird kanonisiert, mit SHA3-256 gehasht, in eine
Hash-Kette eingeschrieben und kann von unabhängigen Tenants gegengeprüft werden.

Designprinzipien: **deterministisch** (gleicher Seed → gleiche Trajektorie →
gleiche Digests), **fail-closed** (ungültige Eingaben werden verworfen, nie
teilweise verarbeitet), **append-only** (keine Update-/Delete-Pfade auf dem
Audit-Layer), **beobachtbar in Echtzeit** (Push statt Polling).

## 2. Schichtenmodell

```
┌─────────────────────────────────────────────────────────────┐
│ Präsentation   public/  Dashboard · Tensor-Labor · Auditor  │
│                Dark-Industrial · Terminal-Typo · SSE-Pulse  │
├─────────────────────────────────────────────────────────────┤
│ Logik          server/tensor.js   PredictiveTensorController│
│                server/auditor.js  Tenant-Cross-Validation   │
│                KETTE12-Isolation: Tenant-Tag je Eintrag     │
├─────────────────────────────────────────────────────────────┤
│ Infrastruktur  server/worm.js     WORM-Hash-Kette (JSONL)   │
│                server/canonical.js  Canonical JSON + SHA3   │
│                server/index.js    HTTP/JSON-API + SSE       │
├─────────────────────────────────────────────────────────────┤
│ Backbone       omega-nexus-kette12 Relay (mTLS, Outbox,     │
│ (extern)       OutboxTimelock L1) · dvp-settlement-prototype│
└─────────────────────────────────────────────────────────────┘
```

### 2.1 Präsentations-Schicht

- Farbsystem: Background `#0A0A0A`, Surface `#1E1E1E`, Accent `#00FF41`,
  Warnung `#FFB000`, Fehler `#FF3B3B`, Text `#D0D0D0`.
- Typografie: Monospace (`ui-monospace, "JetBrains Mono", "Fira Code", monospace`),
  Großbuchstaben-Labels mit Letter-Spacing, keine Icons ohne Textlabel.
- Pulse-Widgets: Server-Sent Events (`GET /api/events`) liefern jede Sekunde
  `pulse` (Tick, Entropie, WORM-Länge, Head-Hash) sowie `worm` bei jedem neuen
  Eintrag. Kein Polling; bei Verbindungsabbruch zeigt die Konsole
  `LINK LOST` (fail-closed Anzeige).
- Ziel: minimale kognitive Reibung — eine Bildschirmseite, drei Panels, keine
  Modaldialoge.

### 2.2 Logik-Schicht

**PredictiveTensorController** (`server/tensor.js`)

- Zustand: Vektor `s ∈ ℝ^d` (Default `d = 8`), L1-normiert, strikt positiv.
- Initialisierung aus `xorshift32(seed)` — vollständig reproduzierbar.
- `step()`: `s'_i = 0.7·s_i + 0.3·s_{(i+1) mod d} + 0.02·(u−0.5)`, clamp ≥ 1e-9,
  Renormierung. Deterministisch durch seed-gebundenen PRNG.
- `apply(M)`: `s' = |M·s|`, gleiche Clamp/Renormierung; Form `d×d`, alle Einträge
  endlich, sonst Fehler (kein Teilzustand).
- Entropie: `H(s) = −Σ s_i log₂ s_i` (Bits), `0 ≤ H ≤ log₂ d`.
- Digest: `SHA3-256(canonical({tick, state: s.map(toFixed(12))}))`.

**Auditor / Tenant-Cross-Validation** (`server/auditor.js`)

- Zwei Tenants liefern unabhängig einen Digest zu einem Subjekt.
  `VERIFIED` ⇔ `digestA == digestB`, sonst `DIVERGENT`.
- Jedes Ergebnis wird als `CROSS_VALIDATION`-Eintrag unter Tenant `AUDITOR`
  in die WORM-Kette geschrieben; `matrixHash` bindet Tenants, Digests und Subjekt.
- `matrix()` aggregiert je Tenant-Paar → Verlässlichkeitsmatrix für
  Hochsicherheitsumgebungen.

**KETTE12-Isolationsmuster**

- Jeder WORM-Eintrag trägt einen `tenant`-Tag; Abfragen sind tenant-filterbar.
- Im Backbone entspricht dies der mTLS-Node-Identität (CN/SAN = `node_id`) und
  der Topic-ACL. Zielbild: 12 Guardians, 8/12-Quorum
  (`dvp-settlement-prototype/docs/security/KETTE12-QUORUM-SPEC.md`).

### 2.3 Infrastruktur-Schicht

**Canonicalisierung** (`server/canonical.js`): deterministisches JSON
(rekursiv sortierte Schlüssel, kein Whitespace, `NaN/Infinity/undefined`
verboten). Kompatibel im Geist mit RFC 8785 (JCS) wie im dvp-Prototyp.

**WORM-Hash-Kette** (`server/worm.js`)

```
entry_n = { seq, ts, tenant, kind, payload, prevHash, hash }
hash_n  = SHA3-256( prevHash_n ‖ canonical({seq, ts, tenant, kind, payload}) )
hash_0  : prevHash = 0x00…00 (64 Hex-Nullen)
```

- Persistenz: JSONL, ausschließlich `appendFile`. Keine Update-/Delete-API.
- `verify()` liest die Datei neu von Platte und rechnet die gesamte Kette nach;
  liefert `{ ok, length, headHash, brokenAt, reason }`. Jeder Verify-Lauf wird
  selbst als `CHAIN_VERIFY`-Eintrag protokolliert.
- Ereignistypen: `SYSTEM · TENSOR_STEP · TENSOR_APPLY · CROSS_VALIDATION · CHAIN_VERIFY`.
- **Verankerung (Zielbild):** Head-Hash periodisch über die KETTE12-Outbox
  (`queue(bytes)` → `OutboxTimelock`) auf L1 und in Rekor (Sigstore) schreiben.
  Damit wird der lokale WORM-Store extern nachprüfbar (Nicht-Überschreibbarkeit
  gegenüber Dritten).

**High-Concurrency Node-Sync:** übernommen aus dem Relay
(Tokio, mTLS-Sessions, Topic-Routing, durable Outbox). TE12 konsumiert
`RelayMessage::Payload`-Frames; Tensor-/Auditor-Jobs sind Topics.

## 3. Funktionale Module

| Modul | Zweck | Implementierung |
|---|---|---|
| Reaktives Dashboard & Pulse | Entropie-Trajektorie live, Head-Hash, Tick, Link-Status | `public/` + `GET /api/events` |
| Tensor-Labor | Schrittweise Evolution, freie `d×d`-Matrix anwenden, Reset per Seed; jede Operation wird protokolliert | `POST /api/tensor/step|apply|reset` |
| Auditor-Modul (KETTE12) | Cross-Validation zweier Tenants, Verlässlichkeitsmatrix, Ketten-Verifikation | `POST /api/auditor/cross-validate`, `GET /api/auditor/matrix`, `GET /api/worm/verify` |
| Kryptografische WORM-Sicherung | Hash-Kette aller Statuswechsel, tenant-filterbare Einsicht | `GET /api/worm?tenant=&limit=` |

## 4. API-Übersicht

| Methode | Pfad | Antwort |
|---|---|---|
| GET | `/api/state` | System, Tensor-Snapshot, Trajektorie, WORM-Head, Auditor-Matrix |
| POST | `/api/tensor/step` | `{ snapshot, entry }` |
| POST | `/api/tensor/apply` `{ matrix }` | `{ snapshot, entry }` / 400 |
| POST | `/api/tensor/reset` `{ seed }` | `{ snapshot, entry }` |
| POST | `/api/auditor/cross-validate` | Ergebnis + WORM-Eintrag |
| GET | `/api/auditor/matrix` | Aggregation |
| GET | `/api/worm` | Einträge |
| GET | `/api/worm/verify` | `{ verify, entry }` |
| GET | `/api/events` | SSE: `state`, `pulse`, `worm` |

## 5. Sicherheitsmodell

- **Fail-closed:** Validierungsfehler → HTTP 400, kein Eintrag, kein
  Zustandswechsel. Ketten-Bruch → `ok:false` mit `brokenAt`.
- **Integrität:** SHA3-256 (FIPS 202) via `node:crypto`. Bestehende
  Digests des Backbones (Keccak-256, SHA-256/JCS, Poseidon) werden als Payload
  eingebettet, nicht ersetzt.
- **Isolation:** Tenant-Tag pro Eintrag; im Zielbild mTLS-Identität als Tenant.
- **Bedrohungen, die der Prototyp NICHT abdeckt:** Löschen/Ersetzen der
  gesamten JSONL-Datei durch einen Host-Admin (nur durch externe Verankerung
  erkennbar), Authentifizierung der HTTP-API (im Backbone via mTLS zu lösen),
  Zeitstempel-Vertrauen (kein RFC-3161-TSA).

## 6. Deployment & Betrieb

```
cd te12-console && npm test && npm start     # http://127.0.0.1:8712
PORT, HOST, DATA_DIR                          # Konfiguration per Umgebung
```

Keine externen Abhängigkeiten (Node ≥ 20, nur Builtins). WORM-Datei:
`$DATA_DIR/worm.jsonl` (Standard `te12-console/data/`).

## 7. Compliance & Standards — Status

| Standard | Bezug | Status |
|---|---|---|
| SEC 17 CFR § 240.17a-4 (f) | Nicht-überschreibbare, nicht-löschbare Aufzeichnung; Verifikation der Aufzeichnungsqualität; Audit-Trail | **ausgelegt für**: Append-only + Hash-Kette + Verify. Offen: externe Verankerung, Aufbewahrungsdauer, D3P-Rolle |
| SOC 2 Type II | Kontrollen über Zeitraum wirksam | **nicht nachgewiesen**; Kontrollkatalog offen |
| ISO/IEC 27001 | ISMS | Pre-Audit-Selbsteinschätzung im dvp-Repo; **nicht zertifiziert** |

Formulierung für Außendarstellung bis zur Zertifizierung: „auf … ausgelegt“,
nicht „erfüllt“. Details: `GAP-ANALYSIS.md` §5/§6.
