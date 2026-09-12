import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
OPENAPI_PATH = REPO_ROOT / "architecture-scaffolding" / "openapi.yaml"


class AuditReportContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.openapi_text = OPENAPI_PATH.read_text(encoding="utf-8")

    def _validate_audit_report(self, payload: dict) -> None:
        required_root = {"report_id", "generated_at", "status", "evidence", "archive"}
        self.assertTrue(required_root.issubset(payload), "missing top-level audit report fields")
        self.assertEqual(payload["status"], "verified")

        evidence = payload["evidence"]
        required_evidence = {
            "commit_sha",
            "artifact_digest",
            "provenance_subject_digest",
            "blake3_root",
            "signature_scheme",
        }
        self.assertTrue(required_evidence.issubset(evidence), "missing evidence fields")
        self.assertEqual(evidence["signature_scheme"], "Ed25519")

        archive = payload["archive"]
        self.assertEqual(archive["mode"], "WORM")

        files = archive["files"]
        expected_files = {
            "vault_manifest": "vault_manifest.json",
            "blake3_root": "blake3_root.json",
            "state_export": "state_export.json",
            "ledger": "ledger.jsonl",
            "sig_meta": "sig_meta.json",
        }
        self.assertEqual(files, expected_files)

        directories = archive["directories"]
        self.assertEqual(directories, {"vault_export": "vault/export/"})

    def test_openapi_tracks_audit_report_contract_shape(self) -> None:
        for snippet in (
            "AuditReport:",
            "required: [report_id, generated_at, status, evidence, archive]",
            "signature_scheme:",
            "const: WORM",
            "directories:",
            "vault_export:",
        ):
            self.assertIn(snippet, self.openapi_text)

    def test_valid_audit_report_payload_matches_contract(self) -> None:
        payload = {
            "report_id": "report-001",
            "generated_at": "2026-09-12T10:10:25Z",
            "status": "verified",
            "evidence": {
                "commit_sha": "abc123",
                "artifact_digest": "sha256:111",
                "provenance_subject_digest": "sha256:111",
                "blake3_root": "b3:222",
                "signature_scheme": "Ed25519",
            },
            "archive": {
                "mode": "WORM",
                "files": {
                    "vault_manifest": "vault_manifest.json",
                    "blake3_root": "blake3_root.json",
                    "state_export": "state_export.json",
                    "ledger": "ledger.jsonl",
                    "sig_meta": "sig_meta.json",
                },
                "directories": {
                    "vault_export": "vault/export/",
                },
            },
        }

        self._validate_audit_report(payload)

    def test_missing_required_audit_report_fields_fail_validation(self) -> None:
        payload = {
            "report_id": "report-001",
            "generated_at": "2026-09-12T10:10:25Z",
            "status": "verified",
            "evidence": {
                "commit_sha": "abc123",
                "artifact_digest": "sha256:111",
                "signature_scheme": "Ed25519",
            },
            "archive": {
                "mode": "WORM",
                "files": {
                    "vault_manifest": "vault_manifest.json",
                    "blake3_root": "blake3_root.json",
                },
            },
        }

        with self.assertRaises((AssertionError, KeyError)):
            self._validate_audit_report(payload)


if __name__ == "__main__":
    unittest.main()
