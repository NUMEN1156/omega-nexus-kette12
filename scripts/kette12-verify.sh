#!/usr/bin/env bash
# ==============================================================================
# KETTE12 DETERMINISTIC EVIDENCE VERIFICATION
#
# Verifies the build evidence produced by .github/workflows/compliance-gate.yml.
#
#   --structural-only  Integrity of the evidence bundle (sealed hashes, JCS
#                      manifest, image digest, Cargo.lock digest, SBOM).
#   --strict           Structural checks plus keyless Sigstore attestation:
#                      the Rekor bundle must verify against the canonical
#                      manifest and the signing identity must be this
#                      repository's workflow on the given ref.
#
# Usage:
#   kette12-verify.sh --evidence-dir DIR (--strict|--structural-only)
#                     --repo OWNER/NAME --ref REF_NAME
#
# Exit codes: 0 pass, 1 verification failure, 2 usage / environment error.
# ==============================================================================
set -euo pipefail

EVIDENCE_DIR=""
MODE=""
REPO=""
REF=""
OIDC_ISSUER="${KETTE12_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"
FAILURES=0

log()  { echo "[KETTE12-VERIFY] $*"; }
ok()   { echo "[KETTE12-VERIFY]   OK   $*"; }
fail() { echo "::error::[KETTE12-VERIFY] FAIL $*"; FAILURES=$((FAILURES + 1)); }
warn() { echo "::warning::[KETTE12-VERIFY] WARN $*"; }
die()  { echo "::error::[KETTE12-VERIFY] $*" >&2; exit 2; }

usage() {
  sed -n '2,/^# =*$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --evidence-dir)    [[ $# -ge 2 ]] || usage; EVIDENCE_DIR="$2"; shift 2 ;;
    --strict)          MODE="strict"; shift ;;
    --structural-only) MODE="structural"; shift ;;
    --repo)            [[ $# -ge 2 ]] || usage; REPO="$2"; shift 2 ;;
    --ref)             [[ $# -ge 2 ]] || usage; REF="$2"; shift 2 ;;
    -h|--help)         usage ;;
    *) echo "::error::unknown argument: $1" >&2; usage ;;
  esac
done

[[ -n "${EVIDENCE_DIR}" ]] || die "--evidence-dir is required"
[[ -n "${MODE}" ]]         || die "one of --strict or --structural-only is required"
[[ -n "${REPO}" ]]         || die "--repo is required"
[[ -n "${REF}" ]]          || die "--ref is required"
[[ -d "${EVIDENCE_DIR}" ]] || die "evidence dir '${EVIDENCE_DIR}' does not exist"
command -v python3 >/dev/null 2>&1 || die "python3 is required"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"
if [[ "${MODE}" == "strict" ]]; then
  command -v cosign >/dev/null 2>&1 || die "cosign is required in --strict mode"
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="$(cd "${EVIDENCE_DIR}" && pwd)"
MANIFEST="${EVIDENCE_DIR}/canonical_manifest.json"
SEALED="${EVIDENCE_DIR}/sealed_hashes.sha256"
BUNDLE="${EVIDENCE_DIR}/rekor-bundle.json"
ATTEST_MODE_FILE="${EVIDENCE_DIR}/attestation.mode"

log "mode=${MODE} repo=${REPO} ref=${REF} evidence=${EVIDENCE_DIR}"

# --- 1. Sealed hashes (TOCTOU seal) -----------------------------------------
log "[1/6] Sealed evidence hashes"
if [[ ! -s "${SEALED}" ]]; then
  fail "sealed_hashes.sha256 missing or empty"
else
  if (cd "${EVIDENCE_DIR}" && LC_ALL=C sha256sum --check --strict --quiet sealed_hashes.sha256); then
    ok "all sealed artifacts match sealed_hashes.sha256"
  else
    fail "sealed_hashes.sha256 does not match evidence contents"
  fi
  for required in canonical_manifest.json image.digest sbom.spdx.json cargo.lock.sha256; do
    if ! grep -qE "[[:space:]]\*?${required}\$" "${SEALED}"; then
      fail "${required} is not covered by sealed_hashes.sha256"
    fi
  done
fi

# --- 2. Canonical manifest --------------------------------------------------
log "[2/6] Canonical RFC 8785 manifest"
MANIFEST_COMMIT=""
MANIFEST_DIGEST=""
MANIFEST_EPOCH=""
if [[ ! -s "${MANIFEST}" ]]; then
  fail "canonical_manifest.json missing or empty"
else
  if MANIFEST_FIELDS="$(python3 - "${MANIFEST}" 2>&1 <<'PY'
import json, sys
path = sys.argv[1]
raw = open(path, "rb").read()
data = json.loads(raw, parse_constant=lambda c: sys.exit("manifest contains non-JSON constant " + c))
if not isinstance(data, dict):
    sys.exit("manifest is not a JSON object")
missing = [k for k in ("commit", "image_digest", "source_date_epoch") if not data.get(k)]
if missing:
    sys.exit("manifest missing fields: " + ", ".join(missing))
try:
    import jcs  # RFC 8785 implementation, installed by the workflow
    canonical = jcs.canonicalize(data)
except ImportError:
    # Without the jcs module, json.dumps is byte-identical to RFC 8785 only for
    # objects of strings, integers, booleans and null with BMP-only keys.
    def walk(v):
        if isinstance(v, float) or (isinstance(v, int) and not isinstance(v, bool) and abs(v) > 2**53 - 1):
            sys.exit("manifest contains a non-interoperable number; python module 'jcs' is required to canonicalize it")
        if isinstance(v, dict):
            for k, x in v.items():
                if any(ord(ch) > 0xFFFF for ch in k):
                    sys.exit("manifest key outside the BMP; python module 'jcs' is required")
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)
    walk(data)
    canonical = json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
if raw not in (canonical, canonical + b"\n"):
    sys.exit("manifest is not in canonical JCS form")
print(data["commit"])
print(data["image_digest"])
print(data["source_date_epoch"])
PY
  )"; then
    { read -r MANIFEST_COMMIT; read -r MANIFEST_DIGEST; read -r MANIFEST_EPOCH; } <<<"${MANIFEST_FIELDS}"
    ok "manifest is canonical (commit=${MANIFEST_COMMIT:0:12} epoch=${MANIFEST_EPOCH})"
  else
    fail "canonical_manifest.json: ${MANIFEST_FIELDS}"
  fi
fi

# --- 3. Image digest --------------------------------------------------------
log "[3/6] Image digest correlation"
if [[ ! -s "${EVIDENCE_DIR}/image.digest" ]]; then
  fail "image.digest missing or empty"
else
  IMAGE_DIGEST="$(tr -d '[:space:]' < "${EVIDENCE_DIR}/image.digest")"
  if [[ ! "${IMAGE_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
    fail "image.digest is not a sha256 digest: '${IMAGE_DIGEST}'"
  elif [[ -n "${MANIFEST_DIGEST}" && "${IMAGE_DIGEST}" != "${MANIFEST_DIGEST}" ]]; then
    fail "image.digest (${IMAGE_DIGEST}) differs from manifest image_digest (${MANIFEST_DIGEST})"
  else
    ok "image digest ${IMAGE_DIGEST:0:19}... consistent"
  fi
fi

# --- 4. Source correlation --------------------------------------------------
log "[4/6] Source tree correlation"
if [[ -n "${MANIFEST_COMMIT}" ]]; then
  if [[ ! "${MANIFEST_COMMIT}" =~ ^[0-9a-f]{40}$ ]]; then
    fail "manifest commit is not a full SHA-1: '${MANIFEST_COMMIT}'"
  elif HEAD_SHA="$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null)"; then
    if [[ "${HEAD_SHA}" == "${MANIFEST_COMMIT}" ]]; then
      ok "manifest commit matches checked-out HEAD"
    elif [[ "${MODE}" == "strict" ]]; then
      fail "manifest commit ${MANIFEST_COMMIT:0:12} != HEAD ${HEAD_SHA:0:12}"
    else
      warn "manifest commit ${MANIFEST_COMMIT:0:12} != HEAD ${HEAD_SHA:0:12} (stale local evidence?)"
    fi
  else
    warn "not a git checkout; skipping HEAD correlation"
  fi
fi
if [[ -s "${EVIDENCE_DIR}/cargo.lock.sha256" ]]; then
  EXPECTED_LOCK="$(awk '{print $1}' "${EVIDENCE_DIR}/cargo.lock.sha256")"
  ACTUAL_LOCK="$(sha256sum "${REPO_ROOT}/Cargo.lock" | awk '{print $1}')"
  if [[ "${EXPECTED_LOCK}" == "${ACTUAL_LOCK}" ]]; then
    ok "Cargo.lock digest matches vendored build input"
  else
    fail "Cargo.lock digest mismatch (evidence ${EXPECTED_LOCK:0:12}, tree ${ACTUAL_LOCK:0:12})"
  fi
else
  warn "cargo.lock.sha256 not present in evidence"
fi
if [[ -n "${MANIFEST_EPOCH}" && ! "${MANIFEST_EPOCH}" =~ ^[0-9]+$ ]]; then
  fail "source_date_epoch is not an integer: '${MANIFEST_EPOCH}'"
fi

# --- 5. SBOM ----------------------------------------------------------------
log "[5/6] SBOM"
if [[ ! -s "${EVIDENCE_DIR}/sbom.spdx.json" ]]; then
  fail "sbom.spdx.json missing or empty"
elif python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get("spdxVersion","").startswith("SPDX-") else 1)' "${EVIDENCE_DIR}/sbom.spdx.json"; then
  ok "SBOM is valid SPDX JSON"
else
  fail "sbom.spdx.json is not an SPDX JSON document"
fi

# --- 6. Attestation ---------------------------------------------------------
log "[6/6] Attestation (${MODE})"
verify_bundle() {
  local repo_re ref_re identity_re
  repo_re="$(printf '%s' "${REPO}" | sed -e 's/[][\.*^$+?(){}|]/\\&/g')"
  ref_re="$(printf '%s' "${REF}" | sed -e 's/[][\.*^$+?(){}|]/\\&/g')"
  identity_re="^https://github\\.com/${repo_re}/\\.github/workflows/[^@]+@refs/(heads|tags)/${ref_re}\$"
  if cosign verify-blob \
      --bundle "${BUNDLE}" \
      --certificate-oidc-issuer "${OIDC_ISSUER}" \
      --certificate-identity-regexp "${identity_re}" \
      "${MANIFEST}" >/dev/null 2>&1; then
    ok "rekor bundle verifies for ${REPO}@${REF} (issuer ${OIDC_ISSUER})"
  else
    fail "cosign verify-blob rejected rekor-bundle.json for ${REPO}@${REF}"
  fi
}
if [[ "${MODE}" == "strict" ]]; then
  if [[ -s "${ATTEST_MODE_FILE}" ]] && grep -q "DRY_RUN" "${ATTEST_MODE_FILE}"; then
    fail "attestation.mode reports DRY_RUN but --strict was requested"
  fi
  if [[ ! -s "${BUNDLE}" ]]; then
    fail "rekor-bundle.json missing or empty (required in --strict mode)"
  elif [[ -s "${MANIFEST}" ]]; then
    verify_bundle
  fi
else
  if [[ -s "${BUNDLE}" && -s "${MANIFEST}" ]]; then
    if command -v cosign >/dev/null 2>&1; then
      verify_bundle
    else
      warn "rekor-bundle.json present but cosign not installed; skipping signature check"
    fi
  elif [[ -s "${ATTEST_MODE_FILE}" ]]; then
    ok "attestation.mode=$(tr -d '[:space:]' < "${ATTEST_MODE_FILE}") (structural run, no bundle expected)"
  else
    warn "neither rekor-bundle.json nor attestation.mode present"
  fi
fi

# --- Result -----------------------------------------------------------------
if [[ "${FAILURES}" -gt 0 ]]; then
  echo "::error::[KETTE12-VERIFY] ${FAILURES} check(s) failed (mode=${MODE})"
  exit 1
fi
log "PASS (mode=${MODE})"
