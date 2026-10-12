#!/usr/bin/env bash
# Create or update the single sticky Mote comment on the PR.
#
# The comment is found by a hidden marker at the start of its body and edited in place on every run,
# so the PR never collects a stack of bot comments:
#   * scanning the repository root (`path: .`): `<!-- mote:summary -->`, which mote's own
#     Markdown begins with;
#   * any other `path` (monorepo jobs): `<!-- mote:summary key=<hash of the path> -->` first, so
#     jobs scanning different directories keep one comment each instead of overwriting each other.
#
# Only a comment written by this token's own identity is ever edited: a human who pastes or quotes
# the report (marker included) keeps their comment. The identity is `GET /user`'s login; that call
# fails for GITHUB_TOKEN and GitHub App installation tokens, whose comments are authored by the
# app's bot account (`github-actions[bot]` for GITHUB_TOKEN), so then only comments by
# github-actions[bot] or another GitHub App bot qualify — never a User. If editing fails (403: not
# ours to edit after all; 404: deleted meanwhile) a new comment is posted instead.
#
# With a read-only token (fork / Dependabot PRs) this step is skipped with a warning; the job summary
# still has the full report.
#
# Inputs (env): PR_NUMBER, IS_FORK, INPUT_PATH, MOTE_MARKDOWN_FILE, MOTE_OVERRIDE_MD,
#               MOTE_EXIT_CODE, MOTE_VERSION, MOTE_ARTIFACT_NAME, GH_TOKEN, GITHUB_*
# Outputs: comment-id, comment-url
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd jq

MAX_BYTES=60000 # comment bodies are limited to 65536 characters
GITHUB_TOKEN_BOT='github-actions[bot]'

PR=${PR_NUMBER:-}
if [ -z "$PR" ]; then
  log "Not a pull request; no PR comment."
  exit 0
fi
case "$PR" in *[!0-9]*) die "PR_NUMBER '$PR' is not a number." ;; esac
if is_true "${IS_FORK:-false}"; then
  notice "Fork pull request: the token is read-only, so no PR comment is posted. See the job summary for the Mote report."
  exit 0
fi
REPO=$(repo_slug)

MOTE_MARKDOWN_FILE=$(to_unix_path "${MOTE_MARKDOWN_FILE:-}")
MOTE_OVERRIDE_MD=$(to_unix_path "${MOTE_OVERRIDE_MD:-}")
export MOTE_MARKDOWN_FILE MOTE_OVERRIDE_MD

work=$(mktemp_dir)

# --- which comment is ours ----------------------------------------------------------------------
SCAN_PATH=$(normalize_rel_path "${INPUT_PATH:-.}")
if [ "$SCAN_PATH" = . ]; then
  MARKER=$MOTE_MARKER
else
  printf 'path=%s\n' "$SCAN_PATH" >"$work/key"
  MARKER="<!-- mote:summary key=$(sha256_of "$work/key" | cut -c1-16) -->"
fi

compose_report "$work/report.md" "$MAX_BYTES"
if [ "$MARKER" != "$MOTE_MARKER" ]; then
  { printf '%s\n' "$MARKER" && cat "$work/report.md"; } >"$work/report.keyed.md"
  mv "$work/report.keyed.md" "$work/report.md"
fi
jq -Rs '{body: .}' "$work/report.md" >"$work/body.json"

skip_readonly() {
  warn "Cannot $1 the PR comment (HTTP $API_STATUS): the token is read-only (fork or Dependabot PR?) or lacks 'pull-requests: write'. The job summary has the full report."
  exit 0
}

SELF_LOGIN=""
if api GET user; then
  SELF_LOGIN=$(jq -r '.login // empty' "$API_RESPONSE_FILE" 2>/dev/null || true)
fi
if [ -n "$SELF_LOGIN" ]; then
  SELF_KNOWN=true
  log "Token identity: $SELF_LOGIN"
else
  SELF_KNOWN=false
  SELF_LOGIN=$GITHUB_TOKEN_BOT
  log "Token identity not readable (GITHUB_TOKEN or app installation token); matching comments by $GITHUB_TOKEN_BOT or another GitHub App bot."
fi

existing=""
if api_list "repos/$REPO/issues/$PR/comments" "$work/comments.json"; then
  existing=$(jq -r --arg m "$MARKER" --arg self "$SELF_LOGIN" --argjson known "$SELF_KNOWN" '
    [ .[] | select((.body // "") | sub("^\\s+"; "") | startswith($m)) ] as $marked
    | [ $marked[] | select(.user.login == $self) ]
      + (if $known then [] else
           [ $marked[] | select(.user.type == "Bot" and .performed_via_github_app != null) ]
         end)
    | first | .id // empty' "$work/comments.json")
else
  is_permission_status && skip_readonly "read"
  warn "Listing PR comments failed (HTTP $API_STATUS); posting a new comment."
fi

post_new() {
  if api POST "repos/$REPO/issues/$PR/comments" "$work/body.json"; then
    log "Posted the Mote comment."
  else
    is_permission_status && skip_readonly "post"
    warn "Posting the PR comment failed (HTTP $API_STATUS): $API_ERROR"
    exit 0
  fi
}

case "$existing" in '' | *[!0-9]*) existing="" ;; esac
if [ -n "$existing" ]; then
  if api PATCH "repos/$REPO/issues/comments/$existing" "$work/body.json"; then
    log "Updated the Mote comment $existing."
  else
    warn "Updating comment $existing failed (HTTP $API_STATUS); posting a new comment instead."
    post_new
  fi
else
  post_new
fi
set_output comment-id "$(jq -r '.id // empty' "$API_RESPONSE_FILE")"
set_output comment-url "$(jq -r '.html_url // empty' "$API_RESPONSE_FILE")"
