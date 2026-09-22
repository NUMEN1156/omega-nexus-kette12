# Rollout-Checkliste für die GitHub-Agent-Suite

Zielrepository: `NUMEN1156/omega-nexus-kette12`

## A. Setup-PR

- [ ] Setup-Branch heißt `chore/github-agent-suite`.
- [ ] Setup-PR zielt auf `main`.
- [ ] Alle Workflow-Dateien liegen unter `.github/workflows/`.
- [ ] `.github/agent-policy.yml` ist enthalten.
- [ ] `scripts/*.sh` sind ausführbar.
- [ ] `resolve-pr32-merge.sh` ist enthalten und ausführbar.
- [ ] Keine Secrets oder privaten Schlüssel sind im Diff enthalten.
- [ ] `permissions:` ist für jeden Workflow minimal gesetzt.
- [ ] Fork-PRs werden nicht automatisch beschrieben oder aktualisiert.
- [ ] Kein Workflow verwendet `pull_request_target` zum Ausführen ungeprüften PR-Codes.
- [ ] Kein Workflow fälscht Required Checks oder Commit-Statuses.
- [ ] Setup-PR wurde von einem Maintainer geprüft.

## B. Environment `merge-approval`

GitHub-Pfad: **Repository → Settings → Environments → New environment**

- [ ] Name exakt `merge-approval`.
- [ ] Required reviewers aktiviert.
- [ ] Nur autorisierte Maintainer oder ein vertrauenswürdiges Maintainer-Team eingetragen.
- [ ] Prevent self-review aktiviert, sofern im Plan verfügbar.
- [ ] Keine Secrets hinterlegt.
- [ ] Deployment branches/tag rules auf den vorgesehenen Ablauf begrenzt.
- [ ] Environment wird nur vom Conflict-Resolver verwendet.

## C. Environment `release-approval`

- [ ] Name exakt `release-approval`.
- [ ] Required reviewers aktiviert.
- [ ] Prevent self-review aktiviert, sofern verfügbar.
- [ ] Keine unnötigen Secrets hinterlegt.
- [ ] Release-/Produktionsbranches als zulässige Referenzen geprüft.
- [ ] Nur Draft-Release oder explizit freigegebene Veröffentlichung erlaubt.

## D. Environment `semantic-review`

- [ ] Name exakt `semantic-review`.
- [ ] Required reviewers aktiviert.
- [ ] Prevent self-review aktiviert, sofern verfügbar.
- [ ] `OPENAI_API_KEY` ausschließlich als Environment-Secret hinterlegt.
- [ ] Optional `OPENAI_BASE_URL` als Environment-Variable geprüft.
- [ ] Kein Standard-Workflow kann auf das Secret zugreifen.
- [ ] Review-Workflow checkt nur vertrauenswürdige Skripte von `main` aus.
- [ ] PR-Diff wird größenbegrenzt und nur zur Review übertragen.

## E. Actions-Grundeinstellungen

Pfad: **Repository → Settings → Actions → General**

- [ ] Default workflow permissions: Read repository contents and packages.
- [ ] Schreibrechte werden nur jobweise explizit erteilt.
- [ ] Actions aus Fork-PRs erhalten keine Secrets.
- [ ] Drittanbieter-Actions sind auf vertrauenswürdige, versionierte Releases begrenzt.
- [ ] Workflow-Änderungen benötigen Review.
- [ ] Branch Protection verlangt erfolgreiche, echte CI-Checks.

## F. Erster harmloser Test-PR

- [ ] Nur README- oder Dokumentationsänderung.
- [ ] Orchestrator startet.
- [ ] Test-Agent beendet die Validierung.
- [ ] Es existiert genau ein Test-Report-Kommentar.
- [ ] Zweiter Lauf aktualisiert denselben Kommentar.
- [ ] Kein Push nach `main`.
- [ ] Kein automatischer Merge.

## G. Resolver-Test

- [ ] Kontrollierter 3-Wege-Konflikt wurde absichtlich erzeugt.
- [ ] Resolver wartet vor Approval in `merge-approval`.
- [ ] Vor Approval kein Commit und kein Push.
- [ ] Nach Approval nur PR-Branch aktualisiert.
- [ ] PR-Head-SHA unmittelbar vor Push geprüft.
- [ ] Fork-PR-Test wurde abgewiesen.
- [ ] Test-PR geschlossen.
- [ ] Test-Branch gelöscht.
- [ ] Testcommit auf `main` über das gehärtete Cleanup-Skript revertiert.
- [ ] Audit-Template ausgefüllt.
