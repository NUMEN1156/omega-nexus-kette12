#!/usr/bin/env bash
# Resolve and validate PR #32 against the current main branch.
#
# Safe defaults:
#   - operates only in the local clone
#   - creates a local merge commit unless --no-commit is supplied
#   - never pushes to GitHub
#
# Usage:
#   ./resolve-pr32-merge.sh /path/to/omega-nexus-kette12
#   ./resolve-pr32-merge.sh /path/to/repo --pr 32 --no-commit
#
# The script assumes that the GitHub CLI is authenticated and that the
# repository is NUMEN1156/omega-nexus-kette12 unless --repo is supplied.

set -Eeuo pipefail
IFS=$'\n\t'

OWNER="NUMEN1156"
REPO="omega-nexus-kette12"
PR_NUMBER=32
BASE_BRANCH="main"
COMMIT_RESULT=1
COMMENT_RESULT=1
RUN_PROJECT_TESTS=1
RUN_MERMAID=1
RUN_OPENAPI=1
REPO_DIR=""

usage() {
  cat <<'USAGE'
Usage: resolve-pr32-merge.sh REPO_DIR [options]

Options:
  --pr N                 Pull request number (default: 32)
  --repo OWNER/REPO      GitHub repository (default: NUMEN1156/omega-nexus-kette12)
  --base BRANCH          Base branch (default: main)
  --no-commit            Resolve and validate, but do not create a merge commit
  --no-comment           Do not create the automatic PR comment
  --skip-tests           Skip cargo/Python project tests
  --skip-mermaid         Skip Mermaid rendering validation
  --skip-openapi         Skip Redocly OpenAPI validation
  -h, --help             Show this help

The script never pushes. Push the resulting local commit only after reviewing it:
  git push origin HEAD
USAGE
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

log() {
  printf '\n==> %s\n' "$*"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "Required command not found: $1"
}

while (($#)); do
  case "$1" in
    --pr)
      (($# >= 2)) || fail "--pr requires a value"
      PR_NUMBER="$2"
      shift 2
      ;;
    --repo)
      (($# >= 2)) || fail "--repo requires OWNER/REPO"
      OWNER="${2%%/*}"
      REPO="${2#*/}"
      [[ -n "$OWNER" && -n "$REPO" && "$2" == */* ]] || fail "Invalid repository: $2"
      shift 2
      ;;
    --base)
      (($# >= 2)) || fail "--base requires a branch"
      BASE_BRANCH="$2"
      shift 2
      ;;
    --no-commit)
      COMMIT_RESULT=0
      shift
      ;;
    --no-comment)
      COMMENT_RESULT=0
      shift
      ;;
    --skip-tests)
      RUN_PROJECT_TESTS=0
      shift
      ;;
    --skip-mermaid)
      RUN_MERMAID=0
      shift
      ;;
    --skip-openapi)
      RUN_OPENAPI=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      fail "Unknown option: $1"
      ;;
    *)
      [[ -z "$REPO_DIR" ]] || fail "Repository directory specified more than once"
      REPO_DIR="$1"
      shift
      ;;
  esac
done

[[ -n "$REPO_DIR" ]] || { usage >&2; exit 2; }
[[ -d "$REPO_DIR/.git" ]] || fail "Not a Git repository: $REPO_DIR"

require_command git
require_command gh
require_command python3

cd "$REPO_DIR"
REMOTE="origin"
REMOTE_REPO="$OWNER/$REPO"

log "Checking repository state"
[[ -z "$(git status --porcelain)" ]] || fail "Working tree is not clean; commit or stash changes first"

log "Reading PR metadata"
HEAD_BRANCH="$(gh pr view "$PR_NUMBER" --repo "$REMOTE_REPO" --json headRefName --jq '.headRefName')"
PR_BASE="$(gh pr view "$PR_NUMBER" --repo "$REMOTE_REPO" --json baseRefName --jq '.baseRefName')"
[[ "$PR_BASE" == "$BASE_BRANCH" ]] || fail "PR base is '$PR_BASE', not '$BASE_BRANCH'"
[[ -n "$HEAD_BRANCH" ]] || fail "Could not determine PR head branch"

git remote get-url "$REMOTE" >/dev/null 2>&1 || fail "Remote '$REMOTE' is not configured"

log "Fetching PR branch and current base branch"
git fetch "$REMOTE" "$BASE_BRANCH"
git fetch "$REMOTE" "pull/$PR_NUMBER/head:refs/remotes/$REMOTE/pr-$PR_NUMBER"

log "Checking out PR branch: $HEAD_BRANCH"
if git show-ref --verify --quiet "refs/heads/$HEAD_BRANCH"; then
  git switch "$HEAD_BRANCH"
else
  git switch --create "$HEAD_BRANCH" "refs/remotes/$REMOTE/pr-$PR_NUMBER"
fi

# Ensure we operate on the exact PR head, not on stale local commits.
EXPECTED_HEAD="$(git rev-parse "refs/remotes/$REMOTE/pr-$PR_NUMBER")"
ACTUAL_HEAD="$(git rev-parse HEAD)"
[[ "$EXPECTED_HEAD" == "$ACTUAL_HEAD" ]] || fail "Local PR branch is not at the current PR head; inspect it before continuing"

MERGE_STARTED=0
cleanup_on_error() {
  local code=$?
  if ((MERGE_STARTED)) && ((code != 0)); then
    echo "Merge failed; aborting local merge to restore the pre-merge branch state." >&2
    git merge --abort >/dev/null 2>&1 || true
  fi
  exit "$code"
}
trap cleanup_on_error ERR

log "Merging $REMOTE/$BASE_BRANCH into $HEAD_BRANCH"
MERGE_STARTED=1
if git merge --no-commit --no-ff "$REMOTE/$BASE_BRANCH"; then
  echo "Merge completed without conflicts."
else
  CONFLICTS="$(git diff --name-only --diff-filter=U || true)"
  EXPECTED_CONFLICTS=$'architecture-scaffolding/architecture_summary.json\narchitecture-scaffolding/diagram.mmd\narchitecture-scaffolding/openapi.yaml'
  [[ "$CONFLICTS" == "$EXPECTED_CONFLICTS" ]] || {
    echo "Unexpected conflict set:" >&2
    printf '%s\n' "$CONFLICTS" >&2
    fail "Refusing automatic resolution because conflicts differ from the known PR #32 set"
  }
  echo "Resolving known conflicts using the current main model plus semantic artifact split."
fi

# The current main branch is authoritative for the compliance architecture and
# OpenAPI model. The PR's useful Mermaid fix is applied separately below.
git restore --source="$REMOTE/$BASE_BRANCH" -- \
  architecture-scaffolding/architecture_summary.json \
  architecture-scaffolding/diagram.mmd \
  architecture-scaffolding/openapi.yaml

log "Splitting the current main Mermaid document into flowchart and sequence files"
python3 - "$REPO_DIR" <<'PY'
from pathlib import Path
import json
import sys

repo = Path(sys.argv[1])
architecture = repo / "architecture-scaffolding"
diagram_path = architecture / "diagram.mmd"
summary_path = architecture / "architecture_summary.json"

if not diagram_path.exists():
    raise SystemExit(f"Missing {diagram_path}")
if not summary_path.exists():
    raise SystemExit(f"Missing {summary_path}")

diagram = diagram_path.read_text(encoding="utf-8")
parts = diagram.split("\n---\n", 1)
flowchart = parts[0].replace(r"\n", "<br/>").rstrip() + "\n"
diagram_path.write_text(flowchart, encoding="utf-8")

if len(parts) == 2:
    sequence_path = architecture / "sequence.mmd"
    sequence = parts[1].replace(r"\n", "<br/>").lstrip()
    sequence_path.write_text(
        "%% NEXUS_11 v0.6.0 + KETTE12 End-to-End-Ablauf\n" + sequence,
        encoding="utf-8",
    )
else:
    sequence_path = architecture / "sequence.mmd"
    if not sequence_path.exists():
        raise SystemExit("No embedded sequence diagram found and sequence.mmd is absent")

# Keep compliance_artifacts reserved for compliance/WORM evidence. Document
# architecture files separately so the schemas do not get mixed semantically.
data = json.loads(summary_path.read_text(encoding="utf-8"))
design = data.setdefault("design_artifacts", [])
entries = {
    "diagram.mmd": "Visualisierung der Systemarchitektur als Flowchart.",
    "sequence.mmd": "Visualisierung des End-to-End-Ablaufs als Sequenzdiagramm.",
    "openapi.yaml": "API-Vertrag für Core, Verification und Audit.",
}
existing = {item.get("name") for item in design if isinstance(item, dict)}
for name, purpose in entries.items():
    if name not in existing:
        design.append({"name": name, "purpose": purpose})
summary_path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY

git add architecture-scaffolding/architecture_summary.json \
        architecture-scaffolding/diagram.mmd \
        architecture-scaffolding/sequence.mmd \
        architecture-scaffolding/openapi.yaml

log "Checking for unresolved conflict markers"
if git grep -n -E '^(<<<<<<<|=======|>>>>>>>)' -- \
  architecture-scaffolding/architecture_summary.json \
  architecture-scaffolding/diagram.mmd \
  architecture-scaffolding/sequence.mmd \
  architecture-scaffolding/openapi.yaml; then
  fail "Conflict markers remain"
fi

log "Validating JSON"
python3 -m json.tool architecture-scaffolding/architecture_summary.json >/dev/null

if ((RUN_OPENAPI)); then
  log "Validating OpenAPI"
  if command -v redocly >/dev/null 2>&1; then
    redocly lint architecture-scaffolding/openapi.yaml
  elif command -v npx >/dev/null 2>&1; then
    npx --yes @redocly/cli lint architecture-scaffolding/openapi.yaml
  else
    echo "WARN: redocly/npx unavailable; OpenAPI validation skipped" >&2
  fi
fi

if ((RUN_MERMAID)); then
  log "Rendering Mermaid diagrams"
  if command -v mmdc >/dev/null 2>&1; then
    mmdc -i architecture-scaffolding/diagram.mmd -o /tmp/omega-diagram.svg
    mmdc -i architecture-scaffolding/sequence.mmd -o /tmp/omega-sequence.svg
  elif command -v npx >/dev/null 2>&1; then
    npx --yes @mermaid-js/mermaid-cli \
      -i architecture-scaffolding/diagram.mmd -o /tmp/omega-diagram.svg
    npx --yes @mermaid-js/mermaid-cli \
      -i architecture-scaffolding/sequence.mmd -o /tmp/omega-sequence.svg
  else
    echo "WARN: mmdc/npx unavailable; Mermaid validation skipped" >&2
  fi
fi

if ((RUN_PROJECT_TESTS)); then
  log "Running project validations"
  if [[ -f arche-omega-relayer/Cargo.toml ]]; then
    cargo fmt --manifest-path arche-omega-relayer/Cargo.toml --all -- --check
    cargo check --manifest-path arche-omega-relayer/Cargo.toml
    cargo test --manifest-path arche-omega-relayer/Cargo.toml
  fi
  if [[ -d Core/tests ]]; then
    python3 -m unittest discover -s Core/tests -p 'test_*.py'
  fi
fi

log "Final diff summary"
git diff --cached --stat
printf '\nChanged files:\n'
git diff --cached --name-status

if ((COMMIT_RESULT)); then
  log "Creating local merge commit"
  git commit -m "Resolve PR #${PR_NUMBER} conflicts with ${BASE_BRANCH}"
  MERGE_COMMIT="$(git rev-parse HEAD)"
  echo
  echo "Local merge commit created. No remote push was performed."
  echo "Review with: git show --stat --oneline HEAD"
  echo "Push manually if desired: git push origin HEAD:${HEAD_BRANCH}"

  if ((COMMENT_RESULT)); then
    log "Creating PR comment"
    COMMENT_FILE="$(mktemp)"
    trap 'rm -f "$COMMENT_FILE"' EXIT
    {
      echo "## Automated merge preparation completed"
      echo
      echo "PR #${PR_NUMBER} was merged locally with \`origin/${BASE_BRANCH}\` into \`${HEAD_BRANCH}\`."
      echo
      echo "- Local merge commit: \`${MERGE_COMMIT}\`"
      echo "- Conflict resolution: current \`${BASE_BRANCH}\` architecture/API model retained"
      echo "- Mermaid flowchart and sequence diagram split into separate files"
      echo "- JSON validation: passed"
      if ((RUN_OPENAPI)); then echo "- OpenAPI validation: requested"; else echo "- OpenAPI validation: skipped"; fi
      if ((RUN_MERMAID)); then echo "- Mermaid rendering validation: requested"; else echo "- Mermaid rendering validation: skipped"; fi
      if ((RUN_PROJECT_TESTS)); then echo "- Project tests: requested"; else echo "- Project tests: skipped"; fi
      echo
      echo "> This commit exists locally only. No push or GitHub merge was performed by the script."
    } > "$COMMENT_FILE"
    if gh pr comment "$PR_NUMBER" --repo "$REMOTE_REPO" --body-file "$COMMENT_FILE"; then
      echo "PR comment created."
    else
      echo "WARN: local merge succeeded, but the PR comment could not be created." >&2
    fi
  else
    echo "PR comment skipped (--no-comment)."
  fi
else
  echo
  echo "Validation succeeded; no commit was created (--no-commit)."
  echo "Review staged changes with: git diff --cached"
  echo "PR comment skipped because no merge commit was created."
fi
