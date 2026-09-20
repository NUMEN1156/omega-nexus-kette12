# TE12 Industrial Console — NeuroLumen Logic Core (Prototyp)

Steuerkonsole für das KETTE12-Backbone: reaktives Dashboard mit Entropie-Pulse,
Tensor-Labor, Auditor-Modul (Tenant-Cross-Validation) und SHA3-256-WORM-Audit-Kette.

```
cd te12-console
npm test        # node:test, keine Abhängigkeiten
npm start       # http://127.0.0.1:8712
```

Konfiguration: `PORT` (8712), `HOST` (127.0.0.1), `DATA_DIR` (`./data`, enthält `worm.jsonl`).

- `docs/ARCHITECTURE.md` — Schichtenmodell, Module, API, Sicherheitsmodell, Compliance-Status
- `docs/GAP-ANALYSIS.md` — Soll/Ist gegen `omega-nexus-kette12` und `dvp-settlement-prototype`
- `server/` — `canonical.js`, `worm.js`, `tensor.js`, `auditor.js`, `index.js`
- `public/` — Konsole (Dark-Industrial, SSE-Pulse)
