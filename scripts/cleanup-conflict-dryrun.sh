#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

REPO=""
PR_NUMBER=""
EXPECTED_BRANCH="test/agent-conflict-base"
MAIN_COMMIT=""
APPLY=false
REVERT_MAIN=false
DELETE_BRANCH=false
CLOSE_PR=false
FORCE_CONFIRM=false

usage() {
  cat <<'USAGE'
Usage:
  cleanup-conflict-dryrun.sh --repo OWNER/REPO --pr NUMBER [options]

Default behavior is a read-only dry-run.

Options:
  --repo OWNER/REPO       Repository name (required)
  --pr NUMBER             Test PR number (required)
  --branch NAME           Expected test branch (default: test/agent-conflict-base)
  --main-commit SHA       Main simulation commit to revert
  --apply                 Permit mutating operations
  --close-pr              Close the test PR
  --delete-branch         Delete the remote test branch
  --revert-main           Revert --main-commit on main and push it
  --yes                   Confirm the complete requested cleanup
  --help                  Show this help

Examples:
  # Safe inspection only:
  cleanup-conflict-dryrun.sh --repo NUMEN1156/omega-nexus-kette12 --pr 35

  # Close PR and delete only the test branch:
  cleanup-conflict-dryrun.sh --repo NUMEN1156/omega-nexus-kette12 --pr 35 \
    --apply --close-pr --delete-branch --yes

  # Full cleanup, including an explicit revert on main:
  cleanup-conflict-dryrun.sh --repo NUMEN1156/omega-nexus-kette12 --pr 35 \
    --main-commit SHA --apply --close-pr --delete-branch --revert-main --yes
USAGE
}

log() { printf '[cleanup] %s\n' "$*"; }
fatal() { printf '[cleanup] ERROR: %s\n' "$*" >&2; exit 1; }

while (($#)); do
  case "$1" in
    --repo) REPO="${2:?Missing repository value}"; shift 2 ;;
    --pr) PR_NUMBER="${2:?Missing PR number}"; shift 2 ;;
    --branch) EXPECTED_BRANCH="${2:?Missing branch value}"; shift 2 ;;
    --main-commit) MAIN_COMMIT="${2:?Missing commit SHA}"; shift 2 ;;
    --apply) APPLY=true; shift ;;
    --revert-main) REVERT_MAIN=true; shift ;;
    --delete-branch) DELETE_BRANCH=true; shift ;;
    --close-pr) CLOSE_PR=true; shift ;;
    --yes) FORCE_CONFIRM=true; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fatal "Unknown option: $1" ;;
  esac
done

[[ -n "$REPO" ]] || fatal '--repo is required'
[[ -n "$PR_NUMBER" ]] || fatal '--pr is required'
[[ "$REPO" =~ ^[^/]+/[^/]+$ ]] || fatal 'Repository must be OWNER/REPO'
[[ "$PR_NUMBER" =~ ^[0-9]+$ ]] || fatal 'PR number must be numeric'

command -v gh >/dev/null || fatal 'gh CLI is required'
command -v git >/dev/null || fatal 'git is required'

json="$(gh pr view "$PR_NUMBER" --repo "$REPO" --json number,state,baseRefName,headRefName,headRepository,headRefOid,mergeCommit,url,title)"
readarray -t metadata < <(python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["state"]); print(d["baseRefName"]); print(d["headRefName"]); print(d["headRepository"]["nameWithOwner"]); print(d["headRefOid"]); print(d.get("mergeCommit",{}).get("oid", ""))' <<<"$json")
STATE="${metadata[0]}"
BASE="${metadata[1]}"
HEAD_BRANCH="${metadata[2]}"
HEAD_REPO="${metadata[3]}"
HEAD_SHA="${metadata[4]}"
MERGE_COMMIT="${metadata[5]}"

[[ "$BASE" == main ]] || fatal "PR base is '$BASE', expected 'main'"
[[ "$HEAD_REPO" == "$REPO" ]] || fatal "Refusing fork PR: $HEAD_REPO"
[[ "$HEAD_BRANCH" == "$EXPECTED_BRANCH" ]] || fatal "PR branch '$HEAD_BRANCH' differs from expected '$EXPECTED_BRANCH'"

log "Repository: $REPO"
log "PR: #$PR_NUMBER ($STATE)"
log "Branch: $HEAD_BRANCH ($HEAD_SHA)"
log "Base: $BASE"
log "Mode: $([[ "$APPLY" == true ]] && echo APPLY || echo DRY-RUN)"

if [[ "$REVERT_MAIN" == true ]]; then
  [[ -n "$MAIN_COMMIT" ]] || fatal '--main-commit is required with --revert-main'
  [[ "$MAIN_COMMIT" =~ ^[0-9a-fA-F]{7,40}$ ]] || fatal '--main-commit must be a Git SHA'
  git fetch origin main --quiet
  git cat-file -e "origin/main^{commit}" || fatal 'origin/main is unavailable'
  git merge-base --is-ancestor "$MAIN_COMMIT" origin/main || fatal "Main commit $MAIN_COMMIT is not an ancestor of origin/main"
  subject="$(git show -s --format=%s "$MAIN_COMMIT" 2>/dev/null || true)"
  [[ "$subject" == *test:* || "$subject" == *conflict* ]] || fatal "Refusing to revert non-test-looking commit: $subject"
  log "Would revert main commit: $MAIN_COMMIT ($subject)"
fi

if [[ "$APPLY" != true ]]; then
  log 'Dry-run complete. No GitHub or Git mutation was performed.'
  exit 0
fi

[[ "$FORCE_CONFIRM" == true ]] || fatal '--apply requires --yes as an explicit confirmation'
[[ "$CLOSE_PR" == true || "$DELETE_BRANCH" == true || "$REVERT_MAIN" == true ]] || fatal '--apply requires at least one cleanup action'

if [[ "$CLOSE_PR" == true ]]; then
  log "Closing PR #$PR_NUMBER"
  gh pr close "$PR_NUMBER" --repo "$REPO" --comment 'Conflict-resolver dry-run cleanup completed; no production merge was performed.'
fi

if [[ "$DELETE_BRANCH" == true ]]; then
  log "Deleting remote branch $HEAD_BRANCH"
  gh api --method DELETE "/repos/$REPO/git/refs/heads/$HEAD_BRANCH"
fi

if [[ "$REVERT_MAIN" == true ]]; then
  log "Reverting $MAIN_COMMIT on main"
  git checkout --detach origin/main
  git revert --no-edit "$MAIN_COMMIT"
  git push origin HEAD:main
fi

log 'Cleanup completed.'
