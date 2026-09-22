#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

PR_NUMBER="${1:?PR number required}"
BODY="${2:?comment body required}"
MARKER="$(printf '%s' "$BODY" | sed -n 's/.*<!-- \([^ ]*\) -->.*/\1/p' | head -n1)"
[[ -n "$MARKER" ]] || { echo 'Comment body must contain an HTML marker.' >&2; exit 2; }

comments="$(gh api --paginate \
  "/repos/${GITHUB_REPOSITORY}/issues/${PR_NUMBER}/comments" \
  --jq '.[] | {id, body}' | \
  jq -r --arg marker "$MARKER" 'select(.body | contains($marker)) | .id')"
comment_id="$(printf '%s\n' "$comments" | head -n1)"
if [[ -n "$comment_id" ]]; then
  gh api --method PATCH "/repos/${GITHUB_REPOSITORY}/issues/comments/${comment_id}" -f body="$BODY" >/dev/null
  echo "Updated comment ${comment_id}."
else
  gh pr comment "$PR_NUMBER" --repo "$GITHUB_REPOSITORY" --body "$BODY"
fi
