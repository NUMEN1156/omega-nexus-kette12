# GitHub Agent Suite

This package provides a guarded-autonomy GitHub agent team for `NUMEN1156/omega-nexus-kette12`.

## Included agents

- `agent-orchestrator.yml`: routes PR events to the test agent and maintains one report comment.
- `agent-test.yml`: reusable merge simulation and validation workflow.
- `agent-issue-triage.yml`: conservative issue labels and triage comment.
- `agent-pr-review.yml`: read-only PR facts artifact.
- `agent-semantic-review.yml`: manual, approval-gated structured LLM review.
- `agent-dependency-audit.yml`: scheduled Rust/Python dependency audit.
- `agent-security-audit.yml`: scheduled CodeQL scan.
- `agent-actions-ops.yml`: workflow failure observation.
- `agent-stale.yml`: stale issue/PR maintenance.
- `agent-release-draft.yml`: manual, approval-gated draft release.
- `agent-conflict-resolver.yml`: approval-gated same-repository branch update.
- `agent-health.yml`: scheduled repository health report.
- `scripts/cleanup-conflict-dryrun.sh`: read-only-by-default cleanup for the conflict test.
- `docs/conflict-dryrun-audit-template.md`: reproducible audit record for the dry-run and teardown.
- `scripts/create-setup-pr.sh`: dry-run-by-default setup branch and setup PR helper.
- `docs/rollout-checklist.md`: click-path and security checklist for all three Environments.
- `docs/first-test-pr-template.md`: harmless first smoke-test PR template.

## Installation

Copy `.github/` and `scripts/` plus `resolve-pr32-merge.sh` to a trusted setup branch, open a setup PR, and merge it after review. Configure the `merge-approval` and `release-approval` environments before enabling write workflows.

The package never reads secrets, changes billing, deletes repositories, fabricates check results, or bypasses required checks. Fork PRs are never updated automatically. The conflict resolver is the only workflow that pushes, and it requires an approval-gated environment.

## Setup branch helper

Run `scripts/create-setup-pr.sh` without flags to inspect the planned changes. It remains read-only until both `--apply` and `--yes` are provided. Add `--create-pr` only when the branch should also open the setup PR:

```bash
./scripts/create-setup-pr.sh --apply --create-pr --yes
```

Review `docs/rollout-checklist.md` before enabling any protected Environment. Use `docs/first-test-pr-template.md` for the first harmless documentation-only smoke test.

## Conflict test cleanup

`scripts/cleanup-conflict-dryrun.sh` is read-only by default. Closing the test PR or deleting its test branch requires `--apply --yes`. Reverting a commit on `main` additionally requires `--revert-main --main-commit SHA`; the script refuses to revert a commit that does not look like a test commit and verifies that it is an ancestor of `origin/main`.

## Semantic review

The semantic reviewer is intentionally manual and protected by the `semantic-review` environment. Configure an `OPENAI_API_KEY` repository or environment secret and, optionally, an `OPENAI_BASE_URL` repository variable for an OpenAI-compatible endpoint. Start `GitHub Agent Semantic Review` with a PR number. The reviewer receives only the PR metadata and a size-limited diff, returns strict JSON, uploads a machine-readable artifact, and updates one marked PR comment. It never edits code, approves, merges, pushes, reads secrets, or changes repository policy.
