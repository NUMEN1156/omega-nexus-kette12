#!/usr/bin/env bash
# ==============================================================================
# KETTE12 ONBOARDING & ENVIRONMENT PRE-FLIGHT CHECK
# ==============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

echo "[1/6] Prüfe kritische KETTE12-Pfade..."
test -d vendor || { echo "::error::'vendor/' fehlt! Führe 'cargo vendor vendor/' aus."; exit 2; }
test -f .cargo/config.toml || { echo "::error::'.cargo/config.toml' fehlt!"; exit 2; }
test -f Cargo.lock || { echo "::error::'Cargo.lock' fehlt!"; exit 2; }

echo "[2/6] Prüfe Code-Formatierung (cargo fmt)..."
cargo fmt --all -- --check

echo "[3/6] Cargo Check (strikter Offline-Modus)..."
cargo check --locked --offline --all-targets

echo "[4/6] Cargo Test (strikter Offline-Modus)..."
cargo test --locked --offline

echo "[5/6] Struktureller Gate-Check (scripts/kette12-verify.sh)..."
EVIDENCE_DIR="${REPO_ROOT}/build/evidence"
if [[ -f "${REPO_ROOT}/scripts/kette12-verify.sh" ]]; then
  chmod +x "${REPO_ROOT}/scripts/kette12-verify.sh"

  if [[ -s "${EVIDENCE_DIR}/canonical_manifest.json" ]]; then
    "${REPO_ROOT}/scripts/kette12-verify.sh" \
      --evidence-dir "${EVIDENCE_DIR}" \
      --structural-only \
      --repo "${GITHUB_REPOSITORY:-local/omega-nexus-kette12}" \
      --ref "${GITHUB_REF_NAME:-feat/onboarding}"
  else
    echo "Info: Keine Build-Evidence unter '${EVIDENCE_DIR}' vorhanden – überspringe Artefaktprüfung vor dem Build."
  fi
else
  echo "Info: 'scripts/kette12-verify.sh' nicht vorhanden – übersprungen."
fi

echo "[6/6] Alle Onboarding-Prüfungen erfolgreich abgeschlossen (OK)."
