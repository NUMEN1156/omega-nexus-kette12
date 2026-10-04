import os
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SIGNER = REPO_ROOT / "scripts" / "cosign-sign-blob-retry.sh"


class CosignSignBlobRetryTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.temp_path = Path(self.temp_dir.name)
        self.count_file = self.temp_path / "calls"
        self.fake_bin = self.temp_path / "bin"
        self.fake_bin.mkdir()
        fake_cosign = self.fake_bin / "cosign"
        fake_cosign.write_text(
            """#!/usr/bin/env bash
set -euo pipefail
count=0
[[ ! -f "${COSIGN_TEST_CALLS}" ]] || count="$(cat "${COSIGN_TEST_CALLS}")"
count=$((count + 1))
echo "${count}" > "${COSIGN_TEST_CALLS}"
if [[ "${COSIGN_TEST_MODE}" == "transient" && "${count}" == "1" ]]; then
  echo "fetching ambient OIDC credentials: invalid character 'u' looking for beginning of value" >&2
  exit 1
fi
if [[ "${COSIGN_TEST_MODE}" == "permanent" ]]; then
  echo "cosign signing failed" >&2
  exit 2
fi
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "--bundle" ]]; then
    printf 'bundle' > "$2"
    exit 0
  fi
  shift
done
exit 2
"""
        )
        fake_cosign.chmod(0o755)
        self.env = os.environ.copy()
        self.env.update(
            {
                "PATH": f"{self.fake_bin}{os.pathsep}{self.env['PATH']}",
                "COSIGN_TEST_CALLS": str(self.count_file),
            }
        )
        self.bundle = self.temp_path / "rekor-bundle.json"
        self.manifest = self.temp_path / "canonical_manifest.json"
        self.manifest.write_text("{}\n")

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_retries_oidc_response_parse_failure_and_publishes_bundle(self) -> None:
        self.env["COSIGN_TEST_MODE"] = "transient"

        result = subprocess.run(
            ["bash", str(SIGNER), str(self.bundle), str(self.manifest)],
            env=self.env,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.count_file.read_text().strip(), "2")
        self.assertEqual(self.bundle.read_text(), "bundle")
        self.assertFalse(self.temp_path.joinpath("rekor-bundle.json.attempt").exists())

    def test_does_not_retry_other_cosign_failures(self) -> None:
        self.env["COSIGN_TEST_MODE"] = "permanent"

        result = subprocess.run(
            ["bash", str(SIGNER), str(self.bundle), str(self.manifest)],
            env=self.env,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.count_file.read_text().strip(), "1")
        self.assertFalse(self.bundle.exists())


if __name__ == "__main__":
    unittest.main()
