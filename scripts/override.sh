#!/usr/bin/env bash
# Decide whether the override label on this PR may waive a failed gate.
#
# GitHub lets anyone with triage (or higher) apply labels, so the label alone proves nothing: we look
# up who applied it and honor it only if their repository role is in INPUT_OVERRIDE_ROLES.
#   * `labeled` event for the override label  -> the event's sender
#   * any other event while the label is on the PR (synchronize, reopened, other labels)
#                                              -> actor of the most recent `labeled` issue event
#
# Inputs (env): INPUT_OVERRIDE_LABEL, INPUT_OVERRIDE_ROLES, HEAD_SHA, PR_NUMBER, MOTE_OUT_DIR,
#               GH_TOKEN, GITHUB_EVENT_PATH, GITHUB_EVENT_NAME, GITHUB_REPOSITORY
# Outputs: override (none|honored|ignored), override-actor, override-role, override-md (file)
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd jq

LABEL=${INPUT_OVERRIDE_LABEL:-mote-override}
ROLES=${INPUT_OVERRIDE_ROLES:-maintain,admin}
OUT_NATIVE=${MOTE_OUT_DIR:?MOTE_OUT_DIR is required}
OUT=$(to_unix_path "$OUT_NATIVE")
MD="$OUT/override.md"
mkdir -p "$OUT"
rm -f "$MD"

finish() { # STATE ACTOR ROLE
  set_output override "$1"
  set_output override-actor "${2:-}"
  set_output override-role "${3:-}"
  if [ -s "$MD" ]; then set_output override-md "$OUT_NATIVE/override.md"; else set_output override-md ""; fi
  exit 0
}

if [ -z "$LABEL" ] || ! is_pr_event; then finish none; fi

EVENT_FILE=$(to_unix_path "${GITHUB_EVENT_PATH:-}")
has_label=$(jq -r --arg l "$LABEL" '[.pull_request.labels[]?.name] | index($l) // empty' "$EVENT_FILE")
if [ -z "$has_label" ]; then finish none; fi

REPO=$(repo_slug)
PR=${PR_NUMBER:-$(event_get .pull_request.number)}
case "$PR" in '' | *[!0-9]*) die "Cannot determine the pull request number (got '$PR')." ;; esac
SHA=${HEAD_SHA:-$(event_get .pull_request.head.sha)}
SHORT=$(printf '%s' "$SHA" | cut -c1-12)
NOW=$(now_utc)

actor=""
applied_at=""
if [ "$(event_get .action)" = labeled ] && [ "$(event_get .label.name)" = "$LABEL" ]; then
  actor=$(event_get .sender.login)
  applied_at=$NOW
else
  events=$(mktemp_file)
  if api_list "repos/$REPO/issues/$PR/events" "$events"; then
    actor=$(jq -r --arg l "$LABEL" \
      '[.[] | select(.event == "labeled" and .label.name == $l)] | last | .actor.login // empty' "$events")
    applied_at=$(jq -r --arg l "$LABEL" \
      '[.[] | select(.event == "labeled" and .label.name == $l)] | last | .created_at // empty' "$events")
  else
    warn "Could not read the PR's issue events (HTTP $API_STATUS); cannot verify who applied '$LABEL'."
  fi
fi

ignore() { # REASON
  log "Override label '$LABEL' ignored: $1"
  {
    printf '> [!WARNING]\n'
    printf '> **Override ignored.** The `%s` label is on this PR, but %s\n' "$LABEL" "$1"
  } >"$MD"
}

if [ -z "$actor" ]; then
  ignore "Mote could not determine who applied it."
  finish ignored
fi

role=""
if api GET "repos/$REPO/collaborators/$(uri_encode "$actor")/permission"; then
  role=$(jq -r '.role_name // .permission // empty' "$API_RESPONSE_FILE")
else
  log "Permission lookup for $actor failed (HTTP $API_STATUS)."
fi

allowed=false
IFS=',' read -r -a role_list <<<"$ROLES"
for r in ${role_list[@]+"${role_list[@]}"}; do
  r=$(printf '%s' "$r" | tr -d '[:space:]')
  if [ -n "$r" ] && [ "$r" = "$role" ]; then allowed=true; fi
done

roles_pretty=$(printf '%s' "$ROLES" | sed 's/[[:space:]]//g; s/,/, /g')
if [ "$allowed" = true ]; then
  log "Override label '$LABEL' honored: applied by $actor (role $role)."
  {
    printf '> [!IMPORTANT]\n'
    printf '> **Quality gate overridden** by @%s (role `%s`) with the `%s` label' "$actor" "$role" "$LABEL"
    if [ -n "$applied_at" ]; then printf ', applied %s' "$applied_at"; fi
    printf '.\n> Honored for commit `%s` at %s. Removing the label restores the gate.\n' "$SHORT" "$NOW"
  } >"$MD"
  finish honored "$actor" "$role"
fi

ignore "it was applied by @$actor (role \`${role:-unknown}\`) and only $roles_pretty may override the gate."
finish ignored "$actor" "$role"
