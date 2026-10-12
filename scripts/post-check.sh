#!/usr/bin/env bash
# Publish the `--github-checks` payload as a Checks API check run on the analyzed commit.
#
# The Checks API accepts at most 50 annotations per request: the run is created (completed) with
# the first 50 and the rest are appended by PATCHing it. Works without GitHub Code Security.
# With a read-only token (fork / Dependabot PRs) creation fails with 403: we fall back to workflow
# command annotations (GitHub shows at most 10 errors + 10 warnings per step) and carry on.
#
# A failed gate that doesn't fail the job — honored override label, or `fail-on-gate: false` — is
# published as `neutral` with the reason in the title, so the check doesn't show red next to a
# green job.
#
# Inputs (env): CHECKS_FILE, HEAD_SHA, OVERRIDE, OVERRIDE_ACTOR, INPUT_FAIL_ON_GATE, GH_TOKEN, GITHUB_*
# Outputs: check-run-id, check-run-url
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd jq

CHUNK=50
SUMMARY_MAX=60000 # API limit is 65535 characters
CHECKS=$(to_unix_path "${CHECKS_FILE:-}")
if [ -z "$CHECKS" ] || [ ! -s "$CHECKS" ]; then
  log "No GitHub checks payload; skipping the check run."
  exit 0
fi
REPO=$(repo_slug)
SHA=${HEAD_SHA:-${GITHUB_SHA:-}}
[ -n "$SHA" ] || die "HEAD_SHA is not set."

work=$(mktemp_dir)

# Normalize the payload once: sanitize annotations (columns are only valid on single-line ranges,
# otherwise the whole request is rejected with 422) and apply an honored override.
enforce=true
is_true "${INPUT_FAIL_ON_GATE:-true}" || enforce=false
jq --arg override "${OVERRIDE:-none}" --arg actor "${OVERRIDE_ACTOR:-}" --argjson max "$SUMMARY_MAX" \
  --argjson enforce "$enforce" '
  def clip($n): if type == "string" and length > $n then .[0:$n] + "\n\n…(truncated)" else . end;
  {
    name: (.name // "Mote"),
    conclusion: (.conclusion // "neutral"),
    title: (.title // "Mote"),
    summary: (.summary // ""),
    annotations: [ (.annotations // [])[]
      | if (.start_line != .end_line) then del(.start_column, .end_column) else . end
      | .end_line = (.end_line // .start_line)
      | .message = ((.message // "") | clip(60000))
      | if .raw_details then .raw_details |= clip(60000) else . end ]
  }
  | if $override == "honored" and .conclusion == "failure" then
      .conclusion = "neutral"
      | .title = ("Gate overridden by @" + $actor + " · " + .title)
      | .summary = ("> **Quality gate overridden** by @" + $actor + " (override label). See the PR comment for the audit record.\n\n" + .summary)
    elif ($enforce | not) and .conclusion == "failure" then
      .conclusion = "neutral"
      | .title = ("Advisory, gate not enforced · " + .title)
      | .summary = ("> **Quality gate failed, not enforced** (`fail-on-gate: false`): this check is informational and does not fail the job.\n\n" + .summary)
    else . end
  | .title |= clip(250)
  | .summary |= clip($max)
' "$CHECKS" >"$work/payload.json"

total=$(jq '.annotations | length' "$work/payload.json")
chunks=$(((total + CHUNK - 1) / CHUNK))
log "Check run: $total annotation(s) in $((chunks > 0 ? chunks : 1)) request(s)"

output_for_chunk() { # INDEX -> {title, summary, annotations[i*50 : i*50+50]}
  jq --argjson i "$1" --argjson n "$CHUNK" \
    '{title, summary, annotations: .annotations[($i * $n):(($i + 1) * $n)]}' "$work/payload.json"
}

# Fallback for read-only tokens: workflow-command annotations (10 errors + 10 warnings max).
# Built entirely in jq, escaped like wc_escape/wc_escape_prop, so paths and messages from the PR
# (tabs, newlines, '::', '%') can neither break a line nor start another workflow command.
fallback_annotations() {
  jq -r '
    def esc: tostring | gsub("%"; "%25") | gsub("\r"; "%0D") | gsub("\n"; "%0A");
    def prop: esc | gsub(":"; "%3A") | gsub(","; "%2C");
    def int: if type == "number" then floor | tostring else "1" end;
    .annotations as $a
    | ([ $a[] | select(.annotation_level == "failure") ][0:10] | map(. + {cmd: "error"}))
      + ([ $a[] | select(.annotation_level == "warning") ][0:10] | map(. + {cmd: "warning"}))
    | .[]
    | "::\(.cmd) file=\(.path // "" | prop),line=\(.start_line | int),endLine=\(.end_line | int),title=\(.title // "" | prop)::\(.message // "" | esc)"
  ' "$work/payload.json"
}

# --- create -------------------------------------------------------------------------------------
output_for_chunk 0 >"$work/output0.json"
jq -n --slurpfile p "$work/payload.json" --slurpfile o "$work/output0.json" \
  --arg sha "$SHA" --arg now "$(now_utc)" --arg url "$(run_url)" --arg ext "${GITHUB_RUN_ID:-}" '
  {
    name: $p[0].name, head_sha: $sha, status: "completed", conclusion: $p[0].conclusion,
    completed_at: $now, details_url: $url, external_id: $ext, output: $o[0]
  }' >"$work/create.json"

if ! api POST "repos/$REPO/check-runs" "$work/create.json"; then
  if is_permission_status; then
    warn "Cannot create the '$(jq -r .name "$work/payload.json")' check run: the token is read-only (fork or Dependabot PR?) or lacks 'checks: write' (HTTP $API_STATUS). Falling back to up to 10+10 inline annotations; the job summary has the full report."
    fallback_annotations
    exit 0
  fi
  if [ "$API_STATUS" = 422 ]; then
    warn "Check run rejected (HTTP 422: ${API_ERROR}); retrying without annotations."
    jq '.output.annotations = []' "$work/create.json" >"$work/create-bare.json"
    if ! api POST "repos/$REPO/check-runs" "$work/create-bare.json"; then
      warn "Could not create the check run (HTTP $API_STATUS): $API_ERROR"
      fallback_annotations
      exit 0
    fi
    fallback_annotations
    chunks=0 # annotations are invalid; don't keep sending them
  else
    warn "Could not create the check run (HTTP $API_STATUS): $API_ERROR"
    fallback_annotations
    exit 0
  fi
fi
id=$(jq -r '.id // empty' "$API_RESPONSE_FILE")
html_url=$(jq -r '.html_url // empty' "$API_RESPONSE_FILE")
[ -n "$id" ] || die "Check run created but no id returned."

# --- append the remaining annotations -----------------------------------------------------------
i=1
while [ "$i" -lt "$chunks" ]; do
  output_for_chunk "$i" | jq '{output: .}' >"$work/patch.json"
  if ! api PATCH "repos/$REPO/check-runs/$id" "$work/patch.json"; then
    warn "Appending annotations batch $((i + 1))/$chunks failed (HTTP $API_STATUS): $API_ERROR"
  fi
  i=$((i + 1))
done

log "Check run $id: $html_url"
set_output check-run-id "$id"
set_output check-run-url "$html_url"
