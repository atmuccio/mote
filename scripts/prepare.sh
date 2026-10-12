#!/usr/bin/env bash
# Resolve the run context: head SHA, PR number, fork status, default-branch status, and the
# new-code base (merge-base) — fetching just enough history for `mote scan --base <sha>`.
#
# Inputs (env): INPUT_PATH, INPUT_BASE, INPUT_CONFIG, GH_TOKEN, GITHUB_* (event payload, repository,
#               ref, sha)
# Outputs: head-sha, pr-number, is-pr, is-fork, is-default-branch, base-ref, base-sha,
#          sarif-ref, sarif-sha, cache-scope
#
# Why the extra fetch: actions/checkout clones at depth 1, and `mote scan --base REV` computes
# merge-base(REV, HEAD). A depth-1 merge-base commit alone is disconnected from a depth-1 HEAD, so we
# deepen HEAD by the compare API's `ahead_by` (+1): every commit on a path from HEAD down to the
# merge-base is "ahead", so that depth always reaches it. Blobs are only what those commits touch.
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd git jq

SRC=$(to_unix_path "${INPUT_PATH:-.}")
[ -d "$SRC" ] || die "path '$SRC' does not exist."
REPO=$(repo_slug)
git_() { git -C "$SRC" "$@"; }

# --- context ----------------------------------------------------------------------------------------
PR_NUMBER=""
IS_PR=false
IS_FORK=false
HEAD_SHA=${GITHUB_SHA:-}
if is_pr_event; then
  IS_PR=true
  PR_NUMBER=$(event_get .pull_request.number)
  case "$PR_NUMBER" in *[!0-9]*) die "Unexpected pull request number '$PR_NUMBER' in the event." ;; esac
  HEAD_SHA=$(event_get .pull_request.head.sha)
  head_repo=$(event_get .pull_request.head.repo.full_name)
  if [ -n "$head_repo" ] && [ "$head_repo" != "$REPO" ]; then IS_FORK=true; fi
fi

LOCAL_HEAD=$(git_ rev-parse HEAD 2>/dev/null || true)
[ -n "$LOCAL_HEAD" ] || die "'$SRC' is not inside a git checkout. Add actions/checkout before this action."
if [ -z "$HEAD_SHA" ]; then HEAD_SHA=$LOCAL_HEAD; fi
if [ "$IS_PR" = true ] && [ "$LOCAL_HEAD" != "$HEAD_SHA" ]; then
  warn "Checked-out commit $LOCAL_HEAD is not the PR head $HEAD_SHA (probably the merge ref). Use actions/checkout with 'ref: \${{ github.event.pull_request.head.sha }}' so annotation line numbers match the PR diff."
fi

# Default branch: saves the analysis cache only there.
DEFAULT_BRANCH=$(event_get .repository.default_branch)
if [ -z "$DEFAULT_BRANCH" ] && api GET "repos/$REPO"; then
  DEFAULT_BRANCH=$(jq -r '.default_branch // empty' "$API_RESPONSE_FILE")
fi
IS_DEFAULT=false
if [ "$IS_PR" = false ] && [ -n "$DEFAULT_BRANCH" ] && [ "${GITHUB_REF:-}" = "refs/heads/$DEFAULT_BRANCH" ]; then
  IS_DEFAULT=true
fi

# SARIF is associated with the commit that was actually analyzed.
if [ "$IS_PR" = true ]; then
  SARIF_REF="refs/pull/$PR_NUMBER/head"
  SARIF_SHA=$HEAD_SHA
else
  SARIF_REF=${GITHUB_REF:-}
  SARIF_SHA=$LOCAL_HEAD
fi

# --- base -------------------------------------------------------------------------------------------
BASE_REF=${INPUT_BASE:-}
if [ -z "$BASE_REF" ] && [ "$IS_PR" = true ]; then BASE_REF=$(event_get .pull_request.base.ref); fi
# The base ends up in git command lines and an API URL: accept only a well-formed ref name or SHA
# (no leading '-', which git would parse as an option; no spaces, '..', control characters, ...).
if [ -n "$BASE_REF" ]; then
  case "$BASE_REF" in
    -*) die "Invalid base '$BASE_REF': must be a branch, tag or commit SHA." ;;
  esac
  git check-ref-format --allow-onelevel "$BASE_REF" >/dev/null 2>&1 ||
    die "Invalid base '$BASE_REF': must be a branch, tag or commit SHA."
fi
is_sha() { printf '%s' "$1" | grep -Eqx '[0-9a-f]{40}|[0-9a-f]{64}'; }

is_ancestor() { git_ merge-base --is-ancestor "$1" "$LOCAL_HEAD" 2>/dev/null; }
has_commit() { git_ cat-file -e "$1^{commit}" 2>/dev/null; }
is_shallow() { [ "$(git_ rev-parse --is-shallow-repository 2>/dev/null)" = true ]; }

BASE_SHA=""
if [ -n "$BASE_REF" ]; then
  AHEAD=""
  if api GET "repos/$REPO/compare/$(uri_encode_path "$BASE_REF")...$LOCAL_HEAD?per_page=1"; then
    BASE_SHA=$(jq -r '.merge_base_commit.sha // empty' "$API_RESPONSE_FILE")
    AHEAD=$(jq -r '.ahead_by // empty' "$API_RESPONSE_FILE")
    if [ -n "$BASE_SHA" ] && ! is_sha "$BASE_SHA"; then
      warn "Compare API returned an unexpected merge-base '$BASE_SHA'; computing the merge-base with git."
      BASE_SHA=""
    fi
  else
    warn "Compare API failed for $BASE_REF...$LOCAL_HEAD (HTTP $API_STATUS); computing the merge-base with git."
  fi

  if [ -n "$BASE_SHA" ]; then
    log "merge-base($BASE_REF, HEAD) = $BASE_SHA (HEAD is $AHEAD commit(s) ahead)"
    if is_shallow && ! is_ancestor "$BASE_SHA"; then
      case "$AHEAD" in
        '' | *[!0-9]*) ;;
        *) git_ fetch --no-tags --quiet --depth="$((AHEAD + 1))" origin "$LOCAL_HEAD" ||
          warn "Could not deepen history to the merge-base." ;;
      esac
    fi
    if ! has_commit "$BASE_SHA"; then
      if is_shallow; then
        git_ fetch --no-tags --quiet --depth=1 origin "$BASE_SHA"
      else
        git_ fetch --no-tags --quiet origin "$BASE_SHA"
      fi
    fi
  else
    # No API: fetch the base branch and full history of HEAD, then ask git.
    if is_shallow; then
      git_ fetch --no-tags --quiet --unshallow origin "+refs/heads/$BASE_REF:refs/remotes/origin/$BASE_REF" ||
        git_ fetch --no-tags --quiet --unshallow origin "$BASE_REF" || true
    else
      git_ fetch --no-tags --quiet origin "+refs/heads/$BASE_REF:refs/remotes/origin/$BASE_REF" ||
        git_ fetch --no-tags --quiet origin "$BASE_REF" || true
    fi
    BASE_SHA=$(git_ merge-base "origin/$BASE_REF" "$LOCAL_HEAD" 2>/dev/null ||
      git_ merge-base "$BASE_REF" "$LOCAL_HEAD" 2>/dev/null || true)
    [ -n "$BASE_SHA" ] || die "Could not determine the merge-base of '$BASE_REF' and HEAD. Check the 'base' input."
  fi
  has_commit "$BASE_SHA" || die "Merge-base $BASE_SHA is not available locally after fetching."
  if ! is_ancestor "$BASE_SHA"; then
    warn "Merge-base $BASE_SHA is not connected to HEAD in this (shallow) clone; mote may not be able to compute new code. Consider 'fetch-depth: 0' on actions/checkout."
  fi
fi

# --- analysis-cache scope -------------------------------------------------------------------------
# Part of the actions/cache key (and restore prefix): jobs scanning different directories of a
# monorepo, or with different configs, must not restore and save over each other's cache.
CONFIG_FILE=$(to_unix_path "${INPUT_CONFIG:-}")
if [ -z "$CONFIG_FILE" ] && [ -f "$SRC/.mote.yaml" ]; then CONFIG_FILE="$SRC/.mote.yaml"; fi
scope_in=$(mktemp_file)
{
  printf 'path=%s\n' "$(normalize_rel_path "${INPUT_PATH:-.}")"
  if [ -n "$CONFIG_FILE" ] && [ -f "$CONFIG_FILE" ]; then
    printf 'config:\n'
    cat "$CONFIG_FILE"
  else
    printf 'config: none\n'
  fi
} >"$scope_in"
CACHE_SCOPE=$(sha256_of "$scope_in" | cut -c1-16)
rm -f "$scope_in"

set_output head-sha "$HEAD_SHA"
set_output pr-number "$PR_NUMBER"
set_output is-pr "$IS_PR"
set_output is-fork "$IS_FORK"
set_output is-default-branch "$IS_DEFAULT"
set_output base-ref "$BASE_REF"
set_output base-sha "$BASE_SHA"
set_output sarif-ref "$SARIF_REF"
set_output sarif-sha "$SARIF_SHA"
set_output cache-scope "$CACHE_SCOPE"
