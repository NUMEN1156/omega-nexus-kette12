# HELL_SCAN v3.5.0 – Technischer Audit- & Testleitfaden

**Status:** RoE- und StGB-202-konform (Hinweis: CREST ist eine Prüforganisation, kein Compliance-Standard.)

## 1. Architektur-Abgleich & Review-Querverweise

| Ref. | Maßnahme |
|------|----------|
| P1-1 | **Kill-Switch:** Das Shared-Memory-Lockfile (`/dev/shm`) wirkt nur node-lokal. Clusterweite Revocation erfolgt über den Operator in Kombination mit einer Egress-Deny-NetworkPolicy. |
| P1-2 | **Consent-Scope-Prüfung:** Vor Missionsstart ist der dokumentierte Auftrag zwingend mit dem technischen Scope abzugleichen. |
| P1-3 | **Egress-Ratenlimit & DNS-Rebinding:** `max_rps` und Ausschlüsse werden auf Egress-Ebene durchgesetzt; die Ziel-IP wird zur Verbindungszeit validiert (Schutz gegen DNS-Rebinding). |
| P3 | **Vier-Augen-Prinzip** bei der Ausstellung von Rules-of-Engagement-(RoE-)Manifesten. |

## 2. Testmodule und Mapping

| Fundklasse | OWASP | MITRE ATT&CK |
|------------|-------|--------------|
| IDOR (Unterfall von Broken Access Control) | A01:2021 | T1078 Valid Accounts; T1530 Data from Cloud Storage |
| BOLA (Begriff der OWASP API Security Top 10 für APIs) | API1:2023 / A01:2021 | T1190 Exploit Public-Facing Application |
| SSRF | A10:2021 | T1190; T1552.005 Cloud Instance Metadata API |
| Injection | A03:2021 | T1190 |
| Fehlkonfiguration | A05:2021 | T1562 Impair Defenses |

Jede Fundklasse wird mit ATT&CK-Technik ausgewiesen, damit das Blue Team Detektionen unmittelbar validieren kann.

## 3. Durchführung

1. RoE-Manifest prüfen (Vier-Augen-Freigabe, Consent-Scope-Abgleich).
2. Mission-Manifest gegen Scope per CEL-Policy validieren.
3. Scan ausführen; Egress-Limits und Ausschlüsse greifen auf Netzwerkebene.
4. Rohbefunde klassifizieren, signieren und berichten.

## 4. Kryptografischer Nachweis

Durchgehende Ed25519-Signaturkette: **RoE → Mission → Report**. Der Report enthält den Inhalts-Hash der Rohbefunde. Findings erhalten stabile SARIF-Fingerprints zur Deduplizierung im Dashboard.

## 5. Operative Bereitschafts-Drills

| # | Drill | Erfolgskriterium |
|---|-------|------------------|
| 1 | Revocation-Latenz auf Node B | < 5 s bis zum Stopp |
| 2 | Egress-Null-Traffic nach Aktivierung der Quarantäne-NetworkPolicy | 0 ausgehende Pakete/Verbindungen |
| 3 | CEL-Reject bei absichtlicher Scope-Abweichung im Missions-Manifest | Mission wird abgelehnt, Audit-Eintrag vorhanden |
| 4 | DNS-Rebinding-Test (Ziel wechselt zur Verbindungszeit auf ausgeschlossene IP) | Verbindung wird blockiert |
| 5 | Ratenlimit-Test | Gemessene Rate ≤ `max_rps` |

Red und Blue Team führen die Drills vor jeder Mission gemeinsam durch und dokumentieren die Ergebnisse.

## 6. Governance, Eskalation & Klassifizierung

- **Eskalation Sofort-Revocation:** Operator/Einsatzleitung → Sicherheitsverantwortliche/r → Auftraggeber (Meldung unverzüglich).
- **Meldeverantwortung:** Die Einsatzleitung meldet Kritisches an den Auftraggeber; das Blue Team erhält ATT&CK-Mapping der Befunde.
- **Klassifizierung:** Rohdaten sind als „VERTRAULICH“ einzustufen, nur im Scope des Auftrags zu speichern, verschlüsselt abzulegen und nach Fristablauf zu löschen.
