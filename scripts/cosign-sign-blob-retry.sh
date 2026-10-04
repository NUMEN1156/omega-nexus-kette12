#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: cosign-sign-blob-retry.sh BUNDLE_PATH MANIFEST_PATH" >&2
  exit 2
fi

BUNDLE_PATH="$1"
MANIFEST_PATH="$2"
ATTEMPT_BUNDLE="${BUNDLE_PATH}.attempt"
OIDC_ERROR="fetching ambient OIDC credentials: invalid character 'u' looking for beginning of value"

rm -f "${BUNDLE_PATH}" "${ATTEMPT_BUNDLE}"
trap 'rm -f "${ATTEMPT_BUNDLE}"' EXIT

for attempt in 1 2 3; do
  rm -f "${ATTEMPT_BUNDLE}"
  if output="$(cosign sign-blob --yes --bundle "${ATTEMPT_BUNDLE}" "${MANIFEST_PATH}" 2>&1)"; then
    printf '%s\n' "${output}"
    if [[ ! -s "${ATTEMPT_BUNDLE}" ]]; then
      echo "cosign succeeded without creating an attestation bundle" >&2
      exit 1
    fi
    mv "${ATTEMPT_BUNDLE}" "${BUNDLE_PATH}"
    exit 0
  else
    status=$?
    printf '%s\n' "${output}" >&2
    if [[ "${output}" != *"${OIDC_ERROR}"* ]] || (( attempt == 3 )); then
      exit "${status}"
    fi
    sleep "${attempt}"
  fi
done
