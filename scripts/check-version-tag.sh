#!/usr/bin/env bash
# ==============================================================================
# KETTE12 RELEASE TAG & CARGO VERSION INTEGRITY CHECK
# ==============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

TAG="${1:-}"
if [[ -z "${TAG}" ]]; then
  echo "::error::Tag-Parameter fehlt!"
  echo "Usage: $0 vX.Y.Z"
  exit 2
fi

[[ "${TAG}" =~ ^v[0-9]+(\.[0-9]+)*$ ]] || {
  echo "::error::Ungültiges Tag-Format: ${TAG} (Erwartet: vX.Y.Z)"
  exit 2
}

if [[ ! -f Cargo.toml ]]; then
  echo "::error::Cargo.toml im Repository-Root nicht gefunden: ${REPO_ROOT}"
  exit 2
fi

CARGO_VERSION="$(awk -F'"' '
  /^\[package\]/ {in_pkg=1; next}
  /^\[/ && $0 !~ /^\[package\]/ {in_pkg=0}
  in_pkg && $1 ~ /^version[[:space:]]*=[[:space:]]*$/ {print $2; exit}
' Cargo.toml)"

[[ -n "${CARGO_VERSION}" ]] || {
  echo "::error::Konnte [package].version nicht aus Cargo.toml extrahieren!"
  exit 2
}

if [[ "v${CARGO_VERSION}" != "${TAG}" ]]; then
  echo "::error::Versions-Mismatch!"
  echo "  Cargo.toml deklariert : ${CARGO_VERSION} (v${CARGO_VERSION})"
  echo "  Ziel-Tag vorgegeben   : ${TAG}"
  exit 2
fi

echo "======================================================"
echo " KETTE12 Version Check: PASS"
echo " Match: Cargo.toml (v${CARGO_VERSION}) == Tag (${TAG})"
echo "======================================================"
exit 0
