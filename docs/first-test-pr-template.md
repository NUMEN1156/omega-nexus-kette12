# Vorlage: Erster Test-PR der GitHub-Agent-Suite

## PR-Titel

```text
chore: validate github agent suite with harmless documentation change
```

## PR-Beschreibung

```markdown
## Zweck

Dieser Pull Request ist ein kontrollierter, harmloser Probelauf für die GitHub-Agent-Suite.

## Änderung

Nur Dokumentation oder README wurde geändert. Es gibt keine Produktionsänderung,
keine Secret-Änderung, kein Deployment und keine Änderung an Branch Protection.

## Erwartete Agentenaktionen

- Orchestrator erkennt den PR.
- Test-Agent führt die read-only-Validierung aus.
- Test-Report wird in genau einem markierten Kommentar gepflegt.
- Semantic Review wird bei Bedarf manuell im geschützten Environment gestartet.
- Kein Agent pusht nach `main`.
- Kein Agent merged den PR automatisch.

## Abnahmekriterien

- [ ] Read-only-Tests erfolgreich
- [ ] Keine Secrets im Log
- [ ] Ein idempotenter Test-Report-Kommentar
- [ ] Kein Commit auf `main`
- [ ] Kein automatischer Merge
- [ ] Audit-Daten dokumentiert

## Rollback

Die Dokumentationsänderung kann durch Schließen des PRs verworfen werden.
Für den ersten Test ist kein Revert auf `main` erforderlich.
```

## Minimaler Testinhalt

```bash
printf '\n\n<!-- github-agent-suite smoke test -->\n' >> README.md
git add README.md
git commit -m 'test: exercise github agent suite with documentation-only change'
git push
```

## Testprotokoll

| Feld | Wert |
|---|---|
| Repository | `NUMEN1156/omega-nexus-kette12` |
| Test-PR | |
| Branch | |
| Head-SHA | |
| Orchestrator-Run | |
| Test-Agent-Run | |
| Semantic-Review-Run | |
| Reviewer | |
| Ergebnis | `PASS` / `FAIL` |

## Nach dem Test

Der Kommentar mit `<!-- github-agent:test-report -->` muss bei einem zweiten Lauf aktualisiert
werden. Es darf kein zweiter Test-Report-Kommentar entstehen. Anschließend kann der PR normal
reviewt und geschlossen oder gemerged werden. Für diesen Smoke-Test ist kein Conflict-Resolver
und kein Revert auf `main` erforderlich.
