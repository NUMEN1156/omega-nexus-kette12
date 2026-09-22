#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

REPO="NUMEN1156/omega-nexus-kette12"
BRANCH="chore/github-agent-suite"
SUITE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKDIR=""
APPLY=false
YES=false
CREATE_PR=false

usage() {
  cat <<'USAGE'
Usage:
  create-setup-pr.sh [options]

Default behavior is a read-only plan. It never clones, pushes, commits, or opens a PR
unless --apply is supplied. --yes is additionally required for the external mutation.

Options:
  --repo OWNER/REPO   Target repository (default: NUMEN1156/omega-nexus-kette12)
  --branch NAME       Setup branch (default: chore/github-agent-suite)
  --workdir PATH      Temporary working directory
  --apply             Clone, populate, commit, and push the setup branch
  --create-pr         Also create the setup PR after pushing
  --yes               Confirm the requested external mutation
  --help              Show this help
USAGE
}

log() { printf '[setup] %s\n' "$*"; }
fatal() { printf '[setup] ERROR: %s\n' "$*" >&2; exit 1; }

while (($#)); do
  case "$1" in
    --repo) REPO="${2:?Missing repository}"; shift 2 ;;
    --branch) BRANCH="${2:?Missing branch}"; shift 2 ;;
    --workdir) WORKDIR="${2:?Missing workdir}"; shift 2 ;;
    --apply) APPLY=true; shift ;;
    --create-pr) CREATE_PR=true; shift ;;
    --yes) YES=true; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fatal "Unknown option: $1" ;;
  esac
done

[[ "$REPO" =~ ^[^/]+/[^/]+$ ]] || fatal 'Repository must be OWNER/REPO'
[[ "$BRANCH" != main ]] || fatal 'Refusing to use main as setup branch'
[[ "$BRANCH" != chore/github-agent-suite ]] && log "Using custom setup branch: $BRANCH"

for command in gh git; do
  command -v "$command" >/dev/null || fatal "$command CLI is required"
done

for required in \
  .github/agent-policy.yml \
  scripts/upsert-pr-comment.sh \
  scripts/semantic_review.py \
  scripts/cleanup-conflict-dryrun.sh \
  resolve-pr32-merge.sh; do
  [[ -f "$SUITE_DIR/$required" ]] || fatal "Suite file missing: $required"
done

if [[ "$APPLY" != true ]]; then
  log 'DRY-RUN: would clone the repository'
  log "DRY-RUN: would create branch $BRANCH"
  log 'DRY-RUN: would copy .github, scripts, docs, README, and resolve-pr32-merge.sh'
  log 'DRY-RUN: would commit the suite and push the branch'
  [[ "$CREATE_PR" == true ]] && log 'DRY-RUN: would create a setup PR against main'
  log 'No external mutation performed. Use --apply --yes to execute.'
  exit 0
fi

[[ "$YES" == true ]] || fatal '--apply requires --yes'
[[ -n "$WORKDIR" ]] || WORKDIR="$(mktemp -d -t github-agent-setup-XXXXXX)"
mkdir -p "$WORKDIR"
REPO_DIR="$WORKDIR/repository"

[[ ! -e "$REPO_DIR" ]] || fatal "Target directory already exists: $REPO_DIR"

log "Cloning $REPO"
gh repo clone "$REPO" "$REPO_DIR"
cd "$REPO_DIR"

git fetch origin main --quiet
git checkout -B "$BRANCH" origin/main

log 'Copying trusted suite files'
cp -a "$SUITE_DIR/.github" .
cp -a "$SUITE_DIR/scripts" .
cp -a "$SUITE_DIR/docs" .
cp "$SUITE_DIR/README.md" .
cp "$SUITE_DIR/resolve-pr32-merge.sh" .
chmod +x scripts/*.sh resolve-pr32-merge.sh

if git diff --quiet -- .github scripts docs README.md resolve-pr32-merge.sh; then
  fatal 'No changes detected; setup branch already contains the suite'
fi

git config user.name 'github-actions[bot]'
git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
git add .github scripts docs README.md resolve-pr32-merge.sh
git commit -m 'ci: add guarded autonomous GitHub agent suite'
git push --set-upstream origin "$BRANCH"

if [[ "$CREATE_PR" == true ]]; then
  gh pr create --repo "$REPO" --base main --head "$BRANCH" \
    --title 'ci: add guarded autonomous GitHub agent suite' \
    --body-file - <<'EOF'
## Summary

Adds the modular GitHub agent suite with guarded automation, read-only validation,
security/dependency audits, issue triage, semantic review, conflict resolution,
and audit/cleanup tooling.

## Safety boundaries

- No secrets, billing, repository deletion, or permission changes are automated.
- Fork PRs are never updated automatically.
- Main-branch merges remain protected by repository policy and human review.
- Write-sensitive workflows require GitHub Environment approvals.
EOF
fi

log "Setup branch ready: $REPO:$BRANCH"
[[ "$CREATE_PR" == true ]] && log 'Setup PR created.'
log "Working copy: $REPO_DIR"
