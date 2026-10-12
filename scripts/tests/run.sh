#!/usr/bin/env bash
# shellcheck disable=SC2030,SC2031 # HOME is changed only inside (...) subshells, on purpose
# Offline tests for the GitHub Action scripts: a fake mote (scripts/tests/fake-mote) and
# MOTE_DRY_RUN=1, which logs GitHub API calls as JSON lines and answers them from fixtures.
# Needs bash, git, jq, curl, tar. Run: bash scripts/tests/run.sh
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SCRIPTS=$(cd "$HERE/.." && pwd)
# shellcheck source=../lib.sh
. "$SCRIPTS/lib.sh" # jq wrapper for Windows, to_unix_path, ...

T=$(mktemp -d "${TMPDIR:-/tmp}/mote-tests.XXXXXX")
trap 'rm -rf "$T"' EXIT
FAKE="$HERE/fake-mote"
chmod +x "$FAKE" 2>/dev/null || true # may be committed without the exec bit from Windows
PASS=0
FAIL=0

ok() {
  PASS=$((PASS + 1))
  printf '  ok   %s\n' "$1"
}
bad() {
  FAIL=$((FAIL + 1))
  printf '  FAIL %s\n' "$1"
  if [ -n "${2:-}" ]; then printf '       %s\n' "$2"; fi
}
eq() { # DESC EXPECTED ACTUAL
  if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "expected [$2], got [$3]"; fi
}
has() { # DESC NEEDLE FILE
  if grep -qF -- "$2" "$3"; then ok "$1"; else bad "$1" "[$2] not found in $3"; fi
}
hasnt() { # DESC NEEDLE FILE
  if grep -qF -- "$2" "$3"; then bad "$1" "[$2] unexpectedly found in $3"; else ok "$1"; fi
}
out() { # NAME — last value written for NAME to $GITHUB_OUTPUT (single-line values)
  grep "^$1=" "$GITHUB_OUTPUT" | tail -n 1 | cut -d= -f2- || true
}
calls() { # JQ_FILTER — query the dry-run API log (one JSON object per call)
  if [ -s "$MOTE_DRY_RUN_LOG" ]; then jq -s -r "$1" "$MOTE_DRY_RUN_LOG"; else jq -n -r '[] | '"$1"; fi
}

# Fresh per-test environment.
reset() {
  local d="$T/case-$1"
  rm -rf "$d"
  mkdir -p "$d/fixtures" "$d/runner-temp" "$d/out"
  CASE=$d
  export RUNNER_TEMP="$d/runner-temp"
  export GITHUB_OUTPUT="$d/github-output"
  export GITHUB_STEP_SUMMARY="$d/step-summary.md"
  export GITHUB_PATH="$d/github-path"
  export MOTE_DRY_RUN=1
  export MOTE_DRY_RUN_LOG="$d/api.jsonl"
  export MOTE_DRY_RUN_FIXTURES="$d/fixtures"
  export MOTE_OUT_DIR="$d/out"
  export GITHUB_REPOSITORY=acme/widgets
  export GITHUB_SERVER_URL=https://github.com
  export GITHUB_RUN_ID=4711
  export GITHUB_EVENT_NAME=pull_request
  export GITHUB_EVENT_PATH="$d/event.json"
  export GITHUB_REF=refs/pull/7/merge
  export GITHUB_SHA=0000000000000000000000000000000000000000
  : >"$GITHUB_OUTPUT"
  : >"$MOTE_DRY_RUN_LOG"
}

pr_event() { # ACTION [LABEL_ON_PR] [EVENT_LABEL] [SENDER] [HEAD_REPO]
  jq -n --arg action "$1" --arg onpr "${2:-}" --arg evlabel "${3:-}" --arg sender "${4:-octocat}" \
    --arg headrepo "${5:-acme/widgets}" '{
      action: $action,
      number: 7,
      pull_request: {
        number: 7,
        head: {sha: "1111111111111111111111111111111111111111", ref: "feature", repo: {full_name: $headrepo}},
        base: {sha: "2222222222222222222222222222222222222222", ref: "main", repo: {full_name: "acme/widgets"}},
        labels: (if $onpr == "" then [{name: "bug"}] else [{name: "bug"}, {name: $onpr}] end)
      },
      repository: {full_name: "acme/widgets", default_branch: "main"},
      sender: {login: $sender}
    } + (if $evlabel == "" then {} else {label: {name: $evlabel}} end)' >"$GITHUB_EVENT_PATH"
}

fixture() { # KEY STATUS [JSON]
  printf '%s\n' "$2" >"$MOTE_DRY_RUN_FIXTURES/$1.status"
  if [ -n "${3:-}" ]; then printf '%s\n' "$3" >"$MOTE_DRY_RUN_FIXTURES/$1.json"; fi
}

# =================================================================================================
echo "scan.sh"
reset scan1
export FAKE_ARGS_LOG="$CASE/args"
MOTE_BIN="$FAKE" FAKE_EXIT=1 BASE_SHA=abc123 MOTE_CACHE_DIR="$CASE/cache" \
  INPUT_EXTRA_ARGS="--include 'src/main/**' --strict" bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1
eq "exit code 1 is captured, step still succeeds" 1 "$(out exit-code)"
eq "gate=failed" failed "$(out gate)"
eq "new-findings counts new, unsuppressed findings" 2 "$(out new-findings)"
eq "results-json output points at the file" "$MOTE_OUT_DIR/results.json" "$(out results-json)"
has "--base is passed" "abc123" "$CASE/args"
has "quoted extra-args stay one argument" "src/main/**" "$CASE/args"
eq "quotes are stripped, nothing evaluated" 0 "$(grep -c "'" "$CASE/args" || true)"
has "--color always is added" "always" "$CASE/args"
has "--github-checks requested" "--github-checks" "$CASE/args"

reset scan2
MOTE_BIN="$FAKE" FAKE_EXIT=0 INPUT_EXTRA_ARGS="--color never" bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1
eq "exit 0 -> gate=passed" passed "$(out gate)"
eq "no base: new-findings = all unsuppressed" 3 "$(out new-findings)"
eq "--color not duplicated when given in extra-args" 1 "$(grep -c -- '--color' "$FAKE_ARGS_LOG")"

reset scan3
MOTE_BIN="$FAKE" FAKE_EXIT=3 bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "scan step itself exits 0 on mote error" 0 "$rc"
eq "exit 3 is captured" 3 "$(out exit-code)"
eq "gate=error" error "$(out gate)"
eq "no results-json output when nothing was written" "" "$(out results-json)"

reset scan-fence
MOTE_BIN="$FAKE" FAKE_EXIT=1 FAKE_STDOUT='::error file=x::forged by a file name' \
  bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1
STOP=$(grep '^::stop-commands::' "$CASE/log" | head -n 1 | sed 's/^::stop-commands:://' || true)
eq "mote output is fenced by ::stop-commands::" "true" \
  "$(printf '%s' "$STOP" | grep -Eq '^mote-[0-9a-f]{8,}' && echo true || echo false)"
eq "fence opens before the forged line and closes after it" "stop forged resume" \
  "$(awk -v t="::$STOP::" '/^::stop-commands::/ {printf "stop "} /forged by a file name/ {printf "forged "} $0 == t {printf "resume"}' "$CASE/log")"
eq "exit code still recorded" 1 "$(out exit-code)"

reset scan-dash
MOTE_BIN="$FAKE" FAKE_EXIT=0 INPUT_PATH=--no-gate INPUT_CONFIG=-cfg.yaml bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1
eq "a path starting with '-' is passed as ./path, never as an option" "./--no-gate" "$(sed -n 2p "$FAKE_ARGS_LOG")"
has "same for config" "./-cfg.yaml" "$FAKE_ARGS_LOG"

reset scan-quotes
rm -f "$FAKE_ARGS_LOG"
MOTE_BIN="$FAKE" INPUT_EXTRA_ARGS="--include 'src/**" bash "$SCRIPTS/scan.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "unbalanced quotes in extra-args fail the step" 1 "$rc"
has "and say why" "Could not parse extra-args" "$CASE/log"
eq "mote is not run without the user's arguments" "false" "$([ -f "$FAKE_ARGS_LOG" ] && echo true || echo false)"
unset FAKE_ARGS_LOG

# =================================================================================================
echo "post-check.sh"
reset check1
pr_event synchronize
"$FAKE" scan --base x --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=1111111111111111111111111111111111111111 \
  bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "120 annotations -> 1 POST + 2 PATCH" "POST PATCH PATCH" "$(calls '[.[].method] | join(" ")')"
eq "chunk sizes 50/50/20" "50 50 20" "$(calls '[.[].body | (.output.annotations | length)] | map(tostring) | join(" ")')"
eq "POST creates a completed run on the PR head" "completed 1111111111111111111111111111111111111111 failure" \
  "$(calls '.[0].body | "\(.status) \(.head_sha) \(.conclusion)"')"
eq "PATCHes target the created run" "repos/acme/widgets/check-runs/4242" "$(calls '.[1].path')"
eq "multi-line annotation loses its columns" "null null" \
  "$(calls '.[0].body.output.annotations[1] | "\(.start_column) \(.end_column)"')"
eq "single-line annotation keeps its columns" "2 7" \
  "$(calls '.[0].body.output.annotations[0] | "\(.start_column) \(.end_column)"')"
eq "every PATCH repeats title and summary" "true" \
  "$(calls '[.[1:][] | .body.output | has("title") and has("summary")] | all')"
eq "check-run-id output" 4242 "$(out check-run-id)"

reset check2
"$FAKE" scan --base x --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc OVERRIDE=honored OVERRIDE_ACTOR=alice \
  bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "honored override -> neutral conclusion" neutral "$(calls '.[0].body.conclusion')"
eq "override named in the title" "true" "$(calls '.[0].body.output.title | startswith("Gate overridden by @alice")')"

reset check3
"$FAKE" scan --base x --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
fixture POST_repos_acme_widgets_check-runs 403 '{"message":"Resource not accessible by integration"}'
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "403 (read-only token) does not fail the step" 0 "$rc"
eq "no PATCH after a failed create" "POST" "$(calls '[.[].method] | join(" ")')"
has "warns about the read-only token" "read-only" "$CASE/log"
eq "fallback: at most 10 ::error annotations" 10 "$(grep -c '^::error file=' "$CASE/log")"
eq "fallback: at most 10 ::warning annotations" 10 "$(grep -c '^::warning file=' "$CASE/log")"
has "fallback escapes the title property and percent in the message" "title=rule-3::Message 3, with: special chars %25" "$CASE/log"

reset check4
FAKE_ANNOTATIONS=0 "$FAKE" scan --base x --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "no annotations -> a single POST" "POST" "$(calls '[.[].method] | join(" ")')"

reset check5
FAKE_ANNOTATIONS=200 "$FAKE" scan --base x --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "200 annotations (the cap) -> 4 requests of 50" "50 50 50 50" \
  "$(calls '[.[].body.output.annotations | length] | map(tostring) | join(" ")')"

reset check-advisory
FAKE_ANNOTATIONS=3 "$FAKE" scan --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc INPUT_FAIL_ON_GATE=false bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "fail-on-gate false + failed gate -> neutral, not a red check" neutral "$(calls '.[0].body.conclusion')"
eq "title says the gate is advisory" "true" "$(calls '.[0].body.output.title | startswith("Advisory, gate not enforced")')"
eq "summary says why" "true" "$(calls '.[0].body.output.summary | contains("not enforced")')"

reset check-advisory-pass
FAKE_EXIT=0 FAKE_ANNOTATIONS=3 "$FAKE" scan --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc INPUT_FAIL_ON_GATE=false bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "fail-on-gate false + passed gate -> stays success" success "$(calls '.[0].body.conclusion')"

reset check-enforced
FAKE_ANNOTATIONS=3 "$FAKE" scan --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc INPUT_FAIL_ON_GATE=true bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "fail-on-gate true + failed gate -> failure" failure "$(calls '.[0].body.conclusion')"

reset check-both
FAKE_ANNOTATIONS=3 "$FAKE" scan --github-checks "$CASE/checks.json" >/dev/null 2>&1 || true
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc INPUT_FAIL_ON_GATE=false OVERRIDE=honored OVERRIDE_ACTOR=alice \
  bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
eq "override and advisory: the override is what the title says" "true" \
  "$(calls '.[0].body.output.title | startswith("Gate overridden by @alice")')"

reset check-inject
jq -n '{name: "Mote", conclusion: "failure", title: "t", summary: "s", annotations: [
  {path: "src/a::b,c.java", start_line: 4, end_line: 4, annotation_level: "failure",
   title: "rule:x,y", message: "first line\n::error::forged\tand a tab %"}]}' >"$CASE/checks.json"
fixture POST_repos_acme_widgets_check-runs 403 '{"message":"Resource not accessible by integration"}'
CHECKS_FILE="$CASE/checks.json" HEAD_SHA=abc bash "$SCRIPTS/post-check.sh" >"$CASE/log" 2>&1
has "fallback annotation escapes path, title and message" \
  "::error file=src/a%3A%3Ab%2Cc.java,line=4,endLine=4,title=rule%3Ax%2Cy::first line%0A::error::forged	and a tab %25" \
  "$CASE/log"
eq "a newline in a message can't start another workflow command" 0 "$(grep -c '^::error::forged' "$CASE/log" || true)"

# =================================================================================================
echo "comment.sh"
BOT='"user": {"login": "github-actions[bot]", "type": "Bot"}, "performed_via_github_app": {"slug": "github-actions"}'
methods() { calls '[.[].method] | join(" ")'; }
posted() { calls '[.[] | select(.method == "POST")][0].body.body'; }

reset comment1
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_user 403 '{"message":"Resource not accessible by integration"}'
fixture GET_repos_acme_widgets_issues_7_comments 200 '[]'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" MOTE_VERSION=0.1.0 MOTE_EXIT_CODE=1 \
  MOTE_ARTIFACT_NAME=mote-results bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "no existing comment -> identity, list, POST" "GET GET POST" "$(methods)"
eq "identity lookup is GET /user" "user" "$(calls '.[0].path')"
eq "POST goes to the PR's comments" "repos/acme/widgets/issues/7/comments" "$(calls '.[2].path')"
eq "body starts with the marker" "true" "$(calls '.[2].body.body | startswith("<!-- mote:summary -->")')"
eq "body has the run link footer" "true" \
  "$(calls '.[2].body.body | contains("https://github.com/acme/widgets/actions/runs/4711")')"
eq "footer names the results artifact" "true" "$(calls '.[2].body.body | contains("results artifact `mote-results`")')"
hasnt "403 on GET /user is not a read-only-token warning" "read-only" "$CASE/log"

reset comment2
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 "[
  {\"id\": 101, \"user\": {\"login\": \"alice\", \"type\": \"User\"}, \"body\": \"LGTM\"},
  {\"id\": 102, \"user\": {\"login\": \"bob\", \"type\": \"User\"}, \"body\": \"> quoting the bot\\n> <!-- mote:summary -->\"},
  {\"id\": 103, \"user\": {\"login\": \"carol\", \"type\": \"User\"}, \"body\": \"<!-- mote:summary -->\\npasted the whole report\"},
  {\"id\": 555, $BOT, \"body\": \"<!-- mote:summary -->\\nold report\"}
]"
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "existing bot comment -> PATCH it (sticky)" "GET GET PATCH" "$(methods)"
eq "PATCH targets the bot comment, not a human's pasted copy" "repos/acme/widgets/issues/comments/555" \
  "$(calls '.[2].path')"
eq "comment-id output" 4242 "$(out comment-id)"

reset comment-hijack
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 '[
  {"id": 103, "user": {"login": "carol", "type": "User"}, "body": "  <!-- mote:summary -->\nfake: gate passed"}
]'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "only a human's marker comment -> never edited, a new one is posted" "GET GET POST" "$(methods)"

reset comment-app
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 '[
  {"id": 103, "user": {"login": "carol", "type": "User"}, "body": "<!-- mote:summary -->\nhuman"},
  {"id": 777, "user": {"login": "acme-ci[bot]", "type": "Bot"}, "performed_via_github_app": {"slug": "acme-ci"}, "body": "<!-- mote:summary -->\nold"}
]'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "app installation token (identity unknown): a GitHub App bot's comment is reused" \
  "repos/acme/widgets/issues/comments/777" "$(calls '[.[] | select(.method == "PATCH")][0].path')"

reset comment-pat
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_user 200 '{"login": "mote-ci", "type": "User"}'
fixture GET_repos_acme_widgets_issues_7_comments 200 "[
  {\"id\": 555, $BOT, \"body\": \"<!-- mote:summary -->\\nfrom another workflow\"},
  {\"id\": 103, \"user\": {\"login\": \"carol\", \"type\": \"User\"}, \"body\": \"<!-- mote:summary -->\\nhuman\"},
  {\"id\": 600, \"user\": {\"login\": \"mote-ci\", \"type\": \"User\"}, \"body\": \"<!-- mote:summary -->\\nours\"}
]"
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "PAT: only the token user's own comment is edited" "repos/acme/widgets/issues/comments/600" \
  "$(calls '[.[] | select(.method == "PATCH")][0].path')"

reset comment-patch403
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 "[{\"id\": 555, $BOT, \"body\": \"<!-- mote:summary -->\\nold\"}]"
fixture PATCH_repos_acme_widgets_issues_comments_555 403 '{"message":"Resource not accessible by integration"}'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "PATCH 403 -> falls back to a new comment" "GET GET PATCH POST" "$(methods)"
eq "PATCH 403 -> step still succeeds" 0 "$rc"
eq "PATCH 403 -> the report is posted" "true" "$(calls '[.[] | select(.method == "POST")][0].body.body | contains("quality gate failed")')"

reset comment-key
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 "[{\"id\": 555, $BOT, \"body\": \"<!-- mote:summary -->\\nroot job\"}]"
PR_NUMBER=7 INPUT_PATH=services/api/ MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" \
  >"$CASE/log" 2>&1
eq "path: services/api doesn't take over the root job's comment" "GET GET POST" "$(methods)"
KEYED=$(posted | head -n 1)
eq "keyed marker first" "true" \
  "$(printf '%s' "$KEYED" | grep -Eqx '<!-- mote:summary key=[0-9a-f]{16} -->' && echo true || echo false)"
eq "bare marker kept on the second line" "<!-- mote:summary -->" "$(posted | sed -n 2p)"
jq -n --arg b "$KEYED
old api report" '[{id: 556, user: {login: "github-actions[bot]", type: "Bot"}, body: $b}]' \
  >"$MOTE_DRY_RUN_FIXTURES/GET_repos_acme_widgets_issues_7_comments.json"
: >"$MOTE_DRY_RUN_LOG"
PR_NUMBER=7 INPUT_PATH=./services/api MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" \
  >"$CASE/log" 2>&1
eq "same directory (spelled ./services/api) -> its own comment is updated" \
  "repos/acme/widgets/issues/comments/556" "$(calls '[.[] | select(.method == "PATCH")][0].path')"
: >"$MOTE_DRY_RUN_LOG"
PR_NUMBER=7 INPUT_PATH=services/web MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" \
  >"$CASE/log" 2>&1
eq "another directory -> another comment" "GET GET POST" "$(methods)"

reset comment3
"$FAKE" scan --markdown "$CASE/summary.md" >/dev/null 2>&1 || true
fixture GET_repos_acme_widgets_issues_7_comments 200 '[]'
fixture POST_repos_acme_widgets_issues_7_comments 403 '{"message":"Resource not accessible by integration"}'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/summary.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "403 on POST does not fail the step" 0 "$rc"
has "warns about the read-only token" "read-only" "$CASE/log"

reset comment4
PR_NUMBER=7 IS_FORK=true bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "fork PR -> no API calls" 0 "$(calls 'length')"

reset comment5
{
  printf '<!-- mote:summary -->\n'
  i=0
  while [ $i -lt 1500 ]; do
    printf '| src/very/long/path/File%04d.java | 12 | java/some-rule | Some message that is long enough |\n' $i
    i=$((i + 1))
  done
} >"$CASE/big.md"
fixture GET_repos_acme_widgets_issues_7_comments 200 '[]'
PR_NUMBER=7 MOTE_MARKDOWN_FILE="$CASE/big.md" bash "$SCRIPTS/comment.sh" >"$CASE/log" 2>&1
eq "oversized report is truncated under 60000 bytes" "true" "$(calls '.[2].body.body | utf8bytelength <= 60000')"
eq "truncation note present" "true" "$(calls '.[2].body.body | contains("Report truncated")')"

reset comment6
MOTE_EXIT_CODE=3 bash "$SCRIPTS/summary.sh" >"$CASE/log" 2>&1
has "summary: failure note when there is no markdown" "did not complete" "$GITHUB_STEP_SUMMARY"
has "summary: marker present" "<!-- mote:summary -->" "$GITHUB_STEP_SUMMARY"
hasnt "summary: no artifact named when artifact-name is empty" "results artifact" "$GITHUB_STEP_SUMMARY"

# =================================================================================================
echo "override.sh"
reset override1
pr_event labeled mote-override mote-override alice
fixture GET_repos_acme_widgets_collaborators_alice_permission 200 '{"permission":"admin","role_name":"admin"}'
PR_NUMBER=7 HEAD_SHA=1111111111111111111111111111111111111111 bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "labeled by an admin -> honored" honored "$(out override)"
eq "actor is the event sender" alice "$(out override-actor)"
eq "labeled event: only the permission lookup is needed" "GET repos/acme/widgets/collaborators/alice/permission" \
  "$(calls '[.[] | "\(.method) \(.path)"] | join(",")')"
has "audit record names the actor" "@alice" "$MOTE_OUT_DIR/override.md"
has "audit record names the commit" "111111111111" "$MOTE_OUT_DIR/override.md"

reset override2
pr_event labeled mote-override mote-override bob
fixture GET_repos_acme_widgets_collaborators_bob_permission 200 '{"permission":"write","role_name":"write"}'
PR_NUMBER=7 bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "labeled by a writer -> ignored" ignored "$(out override)"
has "explains why" "only maintain, admin may override" "$MOTE_OUT_DIR/override.md"

reset override3
pr_event synchronize mote-override "" pusher
fixture GET_repos_acme_widgets_issues_7_events 200 '[
  {"event": "labeled", "label": {"name": "mote-override"}, "actor": {"login": "dave"}, "created_at": "2026-09-01T00:00:00Z"},
  {"event": "unlabeled", "label": {"name": "mote-override"}, "actor": {"login": "dave"}, "created_at": "2026-09-02T00:00:00Z"},
  {"event": "labeled", "label": {"name": "bug"}, "actor": {"login": "erin"}, "created_at": "2026-09-03T00:00:00Z"},
  {"event": "labeled", "label": {"name": "mote-override"}, "actor": {"login": "carol"}, "created_at": "2026-09-04T00:00:00Z"}
]'
fixture GET_repos_acme_widgets_collaborators_carol_permission 200 '{"permission":"maintain","role_name":"maintain"}'
PR_NUMBER=7 bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "synchronize: labeler found from issue events" carol "$(out override-actor)"
eq "synchronize: maintainer's label honored" honored "$(out override)"
has "records when the label was applied" "2026-09-04T00:00:00Z" "$MOTE_OUT_DIR/override.md"

reset override4
pr_event synchronize
PR_NUMBER=7 bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "no label -> none" none "$(out override)"
eq "no label -> no API calls" 0 "$(calls 'length')"

reset override5
pr_event labeled mote-override mote-override some-app[bot]
fixture "GET_repos_acme_widgets_collaborators_some-app_5Bbot_5D_permission" 404 '{"message":"Not Found"}'
PR_NUMBER=7 bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "bot / non-collaborator (404) -> ignored" ignored "$(out override)"
eq "the login is URL-encoded in the API path" "repos/acme/widgets/collaborators/some-app%5Bbot%5D/permission" \
  "$(calls '.[0].path')"
has "the 404 fixture was the one answering" "HTTP 404" "$CASE/log"

# =================================================================================================
echo "finalize.sh"
fin() { # EXIT_CODE FAIL_ON_GATE OVERRIDE -> exit status
  MOTE_EXIT_CODE=$1 INPUT_FAIL_ON_GATE=$2 OVERRIDE=$3 OVERRIDE_ACTOR=alice \
    bash "$SCRIPTS/finalize.sh" >/dev/null 2>&1 && echo 0 || echo $?
}
eq "gate passed -> 0" 0 "$(fin 0 true none)"
eq "gate failed -> 1" 1 "$(fin 1 true none)"
eq "gate failed, fail-on-gate false -> 0" 0 "$(fin 1 false none)"
eq "gate failed, override honored -> 0" 0 "$(fin 1 true honored)"
eq "gate failed, override ignored -> 1" 1 "$(fin 1 true ignored)"
eq "usage error propagates (2)" 2 "$(fin 2 false honored)"
eq "internal error propagates (3)" 3 "$(fin 3 true none)"
eq "scan never ran -> 1" 1 "$(fin '' true none)"
eq "exit code 256 can't turn green (exit 256 == 0)" 1 "$(fin 256 true none)"
eq "exit code 00 can't turn green" 1 "$(fin 00 true none)"
eq "non-numeric exit code -> 1" 1 "$(fin 'x;y' true none)"

# =================================================================================================
echo "install.sh"
reset install1
REL="$CASE/release"
mkdir -p "$REL/pkg"
cp "$FAKE" "$REL/pkg/mote"
chmod +x "$REL/pkg/mote"
tar -czf "$REL/mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz" -C "$REL/pkg" mote
(cd "$REL" && printf '%s  %s\n' "$(sha256sum mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz 2>/dev/null | awk '{print $1}' ||
  shasum -a 256 mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz | awk '{print $1}')" mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz \
  >mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256)
if command -v cygpath >/dev/null 2>&1; then REL_URL="file:///$(cygpath -m "$REL")"; else REL_URL="file://$REL"; fi
export RUNNER_TOOL_CACHE="$CASE/toolcache"
inst() {
  RUNNER_OS=Linux RUNNER_ARCH=X64 MOTE_RELEASE_BASE_URL="$REL_URL" MOTE_ACTION_PATH="$CASE/action" \
    bash "$SCRIPTS/install.sh" >"$CASE/log" 2>&1
}
INPUT_VERSION=v1.2.3 inst && rc=0 || rc=$?
eq "release install succeeds" 0 "$rc"
eq "version output strips the v" 1.2.3 "$(out version)"
eq "installed into the tool cache" "$RUNNER_TOOL_CACHE/mote/1.2.3/x86_64/mote" "$(out bin)"
has "tool cache dir added to GITHUB_PATH (native path)" "$(to_native_path "$RUNNER_TOOL_CACHE/mote/1.2.3/x86_64")" "$GITHUB_PATH"
eq "source=release" release "$(out source)"
: >"$GITHUB_OUTPUT"
INPUT_VERSION=1.2.3 inst
eq "second run hits the tool cache" tool-cache "$(out source)"

: >"$GITHUB_OUTPUT"
mkdir -p "$CASE/action"
printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.3"\nedition = "2021"\n' >"$CASE/action/Cargo.toml"
rm -rf "$RUNNER_TOOL_CACHE"
MOTE_ACTION_REF=v1 inst
eq "major-tag ref -> version from the action's Cargo.toml" 1.2.3 "$(out version)"
: >"$GITHUB_OUTPUT"
rm -rf "$RUNNER_TOOL_CACHE"
MOTE_ACTION_REF=v1.2.3 inst
eq "semver ref -> that version" 1.2.3 "$(out version)"

# `latest` is resolved to a tag first: the asset names carry the version.
: >"$GITHUB_OUTPUT"
rm -rf "$RUNNER_TOOL_CACHE"
printf '{"tag_name": "v1.2.3"}' >"$CASE/fixtures/GET_repos_atmuccio_mote_releases_latest.json"
INPUT_VERSION=latest inst && rc=0 || rc=$?
eq "latest resolves through the API" 0 "$rc"
eq "latest -> the latest release's version" 1.2.3 "$(out version)"
eq "latest downloads the versioned asset" release "$(out source)"
: >"$GITHUB_OUTPUT"
printf '404\n' >"$CASE/fixtures/GET_repos_atmuccio_mote_releases_latest.status"
INPUT_VERSION=latest inst && rc=0 || rc=$?
eq "an unresolvable latest fails" 1 "$rc"
has "unresolvable latest is reported" "Could not resolve the latest release of atmuccio/mote" "$CASE/log"

reset install2
REL2="$CASE/release"
mkdir -p "$REL2"
cp "$REL/mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz" "$REL2/"
printf '%s  mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz\n' \
  0000000000000000000000000000000000000000000000000000000000000000 >"$REL2/mote-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256"
if command -v cygpath >/dev/null 2>&1; then REL_URL="file:///$(cygpath -m "$REL2")"; else REL_URL="file://$REL2"; fi
export RUNNER_TOOL_CACHE="$CASE/toolcache"
INPUT_VERSION=1.2.3 inst && rc=0 || rc=$?
eq "checksum mismatch fails the install" 1 "$rc"
has "checksum mismatch is reported" "Checksum mismatch" "$CASE/log"

reset install3
export RUNNER_TOOL_CACHE="$CASE/toolcache"
INPUT_VERSION='1.2.3/../../evil' inst && rc=0 || rc=$?
eq "a version that isn't semver is rejected" 1 "$rc"
has "invalid version reported" "Invalid version" "$CASE/log"
eq "nothing written outside the tool cache" "false" "$([ -e "$CASE/evil" ] && echo true || echo false)"
INPUT_VERSION=1.2.3 INPUT_REPOSITORY='acme/x y' inst && rc=0 || rc=$?
eq "a malformed repository is rejected" 1 "$rc"
INPUT_VERSION=1.2.3 INPUT_REPOSITORY='../evil' inst && rc=0 || rc=$?
eq "a '..' repository is rejected" 1 "$rc"
INPUT_VERSION=1.2.3 INPUT_MODE=bogus inst && rc=0 || rc=$?
eq "an unknown mode is rejected" 1 "$rc"
has "unknown mode reported" "Invalid mode 'bogus'" "$CASE/log"
unset RUNNER_TOOL_CACHE

# =================================================================================================
echo "prepare.sh (merge-base on a shallow clone)"
reset prepare1
G="$CASE/git"
mkdir -p "$G/home"
printf '[user]\n\tname = t\n\temail = t@example.com\n[core]\n\tautocrlf = false\n[init]\n\tdefaultBranch = main\n' \
  >"$G/home/.gitconfig"
(
  export HOME="$G/home"
  git init -q -b main "$G/work"
  cd "$G/work"
  for i in 1 2 3; do echo "m$i" >"f$i.txt" && git add . && git commit -qm "main $i"; done
  git checkout -q -b feature
  for i in 1 2 3 4; do echo "f$i" >"g$i.txt" && git add . && git commit -qm "feat $i"; done
  git checkout -q main
  echo m4 >f4.txt && git add . && git commit -qm "main 4"
  git checkout -q feature
  git merge -q --no-edit main
  echo f5 >g5.txt && git add . && git commit -qm "feat 5"
  cd "$G"
  git clone -q --bare work origin.git
  git -C origin.git config uploadpack.allowReachableSHA1InWant true
)
HEAD_SHA_T=$(git -C "$G/work" rev-parse feature)
MB_T=$(git -C "$G/work" merge-base main feature)
AHEAD_T=$(git -C "$G/work" rev-list --count main..feature)
if command -v cygpath >/dev/null 2>&1; then ORIGIN_URL="file:///$(cygpath -m "$G/origin.git")"; else ORIGIN_URL="file://$G/origin.git"; fi
(
  export HOME="$G/home"
  git init -q "$G/clone"
  git -C "$G/clone" remote add origin "$ORIGIN_URL"
  git -C "$G/clone" fetch -q --no-tags --depth=1 origin "$HEAD_SHA_T"
  git -C "$G/clone" checkout -q --detach FETCH_HEAD
)
pr_event synchronize
jq --arg h "$HEAD_SHA_T" '.pull_request.head.sha = $h' "$GITHUB_EVENT_PATH" >"$CASE/ev" && mv "$CASE/ev" "$GITHUB_EVENT_PATH"
fixture "GET_repos_acme_widgets_compare_main...$HEAD_SHA_T" 200 \
  "{\"merge_base_commit\": {\"sha\": \"$MB_T\"}, \"ahead_by\": $AHEAD_T}"
eq "precondition: clone is shallow and disconnected" "false" \
  "$(HOME="$G/home" git -C "$G/clone" merge-base --is-ancestor "$MB_T" HEAD 2>/dev/null && echo true || echo false)"
HOME="$G/home" INPUT_PATH="$G/clone" bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "prepare succeeds" 0 "$rc"
eq "base-sha is the compare API merge-base" "$MB_T" "$(out base-sha)"
eq "merge-base is now an ancestor of HEAD" "true" \
  "$(HOME="$G/home" git -C "$G/clone" merge-base --is-ancestor "$MB_T" HEAD && echo true || echo false)"
eq "is-pr" true "$(out is-pr)"
eq "is-fork false for same-repo PR" false "$(out is-fork)"
eq "head-sha from the event" "$HEAD_SHA_T" "$(out head-sha)"
eq "SARIF ref is the PR head ref" refs/pull/7/head "$(out sarif-ref)"
eq "PR runs never save the cache" false "$(out is-default-branch)"

reset prepare2
export GITHUB_EVENT_NAME=push GITHUB_REF=refs/heads/main GITHUB_SHA="$HEAD_SHA_T"
jq -n '{repository: {full_name: "acme/widgets", default_branch: "main"}}' >"$GITHUB_EVENT_PATH"
HOME="$G/home" INPUT_PATH="$G/clone" bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1
eq "push to default branch -> is-default-branch" true "$(out is-default-branch)"
eq "push without base input -> no base" "" "$(out base-sha)"
eq "push: no API calls" 0 "$(calls 'length')"

reset prepare3
pr_event synchronize "" "" octocat someone/widgets
jq --arg h "$HEAD_SHA_T" '.pull_request.head.sha = $h' "$GITHUB_EVENT_PATH" >"$CASE/ev" && mv "$CASE/ev" "$GITHUB_EVENT_PATH"
fixture "GET_repos_acme_widgets_compare_main...$HEAD_SHA_T" 200 \
  "{\"merge_base_commit\": {\"sha\": \"$MB_T\"}, \"ahead_by\": $AHEAD_T}"
HOME="$G/home" INPUT_PATH="$G/clone" bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1
eq "fork PR detected" true "$(out is-fork)"

reset prepare-encode
pr_event synchronize
jq --arg h "$HEAD_SHA_T" '.pull_request.head.sha = $h' "$GITHUB_EVENT_PATH" >"$CASE/ev" && mv "$CASE/ev" "$GITHUB_EVENT_PATH"
fixture "GET_repos_acme_widgets_compare_release_x_7Brepo_7D_231...$HEAD_SHA_T" 200 \
  "{\"merge_base_commit\": {\"sha\": \"$MB_T\"}, \"ahead_by\": $AHEAD_T}"
HOME="$G/home" INPUT_PATH="$G/clone" INPUT_BASE='release/x{repo}#1' bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1 &&
  rc=0 || rc=$?
eq "base with URL/placeholder characters: prepare succeeds" 0 "$rc"
eq "base is URL-encoded in the compare path ('/' kept)" \
  "repos/acme/widgets/compare/release/x%7Brepo%7D%231...$HEAD_SHA_T?per_page=1" "$(calls '.[0].path')"
eq "and the merge-base comes from that call" "$MB_T" "$(out base-sha)"

reset prepare-badbase
pr_event synchronize
for b in '--upload-pack=touch pwned' 'main..evil' 'a b'; do
  HOME="$G/home" INPUT_PATH="$G/clone" INPUT_BASE="$b" bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1 && rc=0 || rc=$?
  eq "base '$b' is rejected" 1 "$rc"
done
has "and reported" "Invalid base" "$CASE/log"
eq "rejected bases make no API calls" 0 "$(calls 'length')"
eq "nothing ran as an option" "false" "$([ -e pwned ] || [ -e "$G/clone/pwned" ] && echo true || echo false)"

reset prepare-scope
export GITHUB_EVENT_NAME=push GITHUB_REF=refs/heads/main GITHUB_SHA="$HEAD_SHA_T"
jq -n '{repository: {full_name: "acme/widgets", default_branch: "main"}}' >"$GITHUB_EVENT_PATH"
mkdir -p "$G/clone/sub"
printf 'gate: {}\n' >"$CASE/a.yaml"
printf 'gate: {new_coverage: 80}\n' >"$CASE/b.yaml"
scope() { # PATH [CONFIG] -> cache-scope output
  : >"$GITHUB_OUTPUT"
  HOME="$G/home" INPUT_PATH="$1" INPUT_CONFIG="${2:-}" bash "$SCRIPTS/prepare.sh" >"$CASE/log" 2>&1 || true
  out cache-scope
}
S_ROOT=$(scope "$G/clone")
eq "cache-scope is 16 hex chars" "true" "$(printf '%s' "$S_ROOT" | grep -Eqx '[0-9a-f]{16}' && echo true || echo false)"
eq "trailing slash: same scope" "$S_ROOT" "$(scope "$G/clone/")"
S_SUB=$(scope "$G/clone/sub")
eq "another directory: another scope" "true" "$([ "$S_SUB" != "$S_ROOT" ] && echo true || echo false)"
S_A=$(scope "$G/clone" "$CASE/a.yaml")
S_B=$(scope "$G/clone" "$CASE/b.yaml")
eq "config contents change the scope" "true" \
  "$([ "$S_A" != "$S_B" ] && [ "$S_A" != "$S_ROOT" ] && echo true || echo false)"
eq "same config contents: same scope" "$S_A" "$(scope "$G/clone" "$CASE/a.yaml")"
rmdir "$G/clone/sub"

# =================================================================================================
echo "gate.sh (mode: gate)"
reset gate1
pr_event synchronize
mkdir -p "$CASE/ws/mote-results" "$CASE/ws/svc"
printf '{}\n' >"$CASE/ws/mote-results/results.json"
printf 'gate: {}\n' >"$CASE/ws/svc/.mote.yaml"
export FAKE_ARGS_LOG="$CASE/args"
(cd "$CASE/ws" && MOTE_BIN="$FAKE" FAKE_GATE_EXIT=1 INPUT_GATE_ONLY=coverage INPUT_PATH=svc \
  INPUT_COVERAGE=$'coverage-reports/**/jacoco.xml\n\n   coverage-reports/**/lcov.info  \n' \
  bash "$SCRIPTS/gate.sh") >"$CASE/log" 2>&1 && rc=0 || rc=$?
eq "gate step itself exits 0 on a failed gate" 0 "$rc"
eq "exit-code 1 recorded" 1 "$(out exit-code)"
eq "gate=failed" failed "$(out gate)"
eq "argv: results, markdown, --only coverage, config of path, two coverage globs" \
  "gate --results mote-results/results.json --format markdown --only coverage --config svc/.mote.yaml --coverage coverage-reports/**/jacoco.xml --coverage coverage-reports/**/lcov.info" \
  "$(tr '\n' ' ' <"$FAKE_ARGS_LOG" | sed 's/ $//')"
eq "markdown output is the captured verdict" "$MOTE_OUT_DIR/gate.md" "$(out markdown)"
has "verdict captured from stdout" "quality gate failed (coverage)" "$MOTE_OUT_DIR/gate.md"
hasnt "diagnostics (stderr) stay out of the report" "evaluating the gate" "$MOTE_OUT_DIR/gate.md"
has "diagnostics go to the log" "evaluating the gate" "$CASE/log"
eq "PR context from the event" "true 7 1111111111111111111111111111111111111111" \
  "$(out is-pr) $(out pr-number) $(out head-sha)"

# The documented coverage job: failed gate + maintainer's override label -> override.sh honors it
# and finalize.sh passes, exactly like the scan job.
pr_event labeled mote-override mote-override alice
fixture GET_repos_acme_widgets_collaborators_alice_permission 200 '{"permission":"admin","role_name":"admin"}'
PR_NUMBER=$(out pr-number) HEAD_SHA=$(out head-sha) bash "$SCRIPTS/override.sh" >"$CASE/log" 2>&1
eq "coverage job: override label honored" honored "$(out override)"
eq "coverage job: the job passes" 0 "$(MOTE_EXIT_CODE=1 INPUT_FAIL_ON_GATE=true OVERRIDE=honored \
  OVERRIDE_ACTOR=alice bash "$SCRIPTS/finalize.sh" >/dev/null 2>&1 && echo 0 || echo $?)"
MOTE_MODE=gate MOTE_MARKDOWN_FILE="$MOTE_OUT_DIR/gate.md" MOTE_OVERRIDE_MD="$MOTE_OUT_DIR/override.md" \
  MOTE_EXIT_CODE=1 bash "$SCRIPTS/summary.sh" >"$CASE/log" 2>&1
has "coverage job summary: the verdict" "coverage on new code" "$GITHUB_STEP_SUMMARY"
has "coverage job summary: the override record" "Quality gate overridden" "$GITHUB_STEP_SUMMARY"
hasnt "coverage job summary: no results artifact claimed" "results artifact" "$GITHUB_STEP_SUMMARY"

reset gate2
(cd "$CASE" && MOTE_BIN="$FAKE" INPUT_RESULTS=missing/results.json bash "$SCRIPTS/gate.sh") >"$CASE/log" 2>&1
eq "missing results file -> exit-code 2 (always fails)" 2 "$(out exit-code)"
has "explains how to get the results" "download-artifact" "$CASE/log"
MOTE_MODE=gate MOTE_EXIT_CODE=2 bash "$SCRIPTS/summary.sh" >"$CASE/log" 2>&1
has "gate-mode failure note" "the gate evaluation did not complete" "$GITHUB_STEP_SUMMARY"

reset gate3
export GITHUB_EVENT_NAME=push
mkdir -p "$CASE/ws"
printf '{}\n' >"$CASE/ws/r.json"
(cd "$CASE/ws" && MOTE_BIN="$FAKE" INPUT_RESULTS=r.json INPUT_GATE_ONLY='' bash "$SCRIPTS/gate.sh") \
  >"$CASE/log" 2>&1
eq "empty gate-only -> whole gate (no --only)" 0 "$(grep -c -- '--only' "$FAKE_ARGS_LOG" || true)"
eq "passed" passed "$(out gate)"
eq "push -> not a PR (override step is skipped)" false "$(out is-pr)"
: >"$GITHUB_OUTPUT"
(cd "$CASE/ws" && MOTE_BIN="$FAKE" INPUT_RESULTS=r.json INPUT_GATE_ONLY='coverage, worsened' \
  bash "$SCRIPTS/gate.sh") >"$CASE/log" 2>&1
eq "gate-only list -> one comma-separated --only" "--only coverage,worsened" \
  "$(grep -A1 -- '--only' "$FAKE_ARGS_LOG" | tr '\n' ' ' | sed 's/ $//')"
eq "accepted by mote" passed "$(out gate)"
: >"$GITHUB_OUTPUT"
(cd "$CASE/ws" && MOTE_BIN="$FAKE" INPUT_RESULTS=r.json INPUT_GATE_ONLY=bogus bash "$SCRIPTS/gate.sh") \
  >"$CASE/log" 2>&1
eq "unknown condition -> mote usage error (2), gate=error" "2 error" "$(out exit-code) $(out gate)"
unset FAKE_ARGS_LOG

# =================================================================================================
echo "lib.sh"
ghes() { # SERVER_URL [ENV...] -> "GH_HOST|GH_ENTERPRISE_TOKEN"
  local url=$1
  shift
  env -u GH_HOST -u GH_ENTERPRISE_TOKEN GITHUB_SERVER_URL="$url" GH_TOKEN=tok "$@" \
    bash -c '. "$1/lib.sh"; printf "%s|%s" "${GH_HOST:-}" "${GH_ENTERPRISE_TOKEN:-}"' _ "$SCRIPTS"
}
eq "github.com: no GH_HOST / GH_ENTERPRISE_TOKEN" "|" "$(ghes https://github.com)"
eq "GHES: host (with port) and the token for gh" "ghes.example.com:8443|tok" "$(ghes https://GHES.example.com:8443/)"
eq "GHES: an explicit GH_ENTERPRISE_TOKEN / GH_HOST wins" "other.host|mine" \
  "$(ghes https://ghes.example.com GH_ENTERPRISE_TOKEN=mine GH_HOST=other.host)"

eq "uri_encode: a bot login" "some-app%5Bbot%5D" "$(uri_encode 'some-app[bot]')"
eq "uri_encode_path keeps '/', encodes the rest" "feat/x%7Brepo%7D%231%20%25" "$(uri_encode_path 'feat/x{repo}#1 %')"
eq "normalize_rel_path" ". . a/b a/b" \
  "$(normalize_rel_path '') $(normalize_rel_path './') $(normalize_rel_path './a/b//') $(normalize_rel_path 'a\b')"

reset lib-output
set_output multi "$(printf 'line1\nMOTE_EOF_\nline3')"
DELIM=$(sed -n '1s/^multi<<//p' "$GITHUB_OUTPUT")
eq "multi-line output: delimiter framed" "$DELIM" "$(tail -n 1 "$GITHUB_OUTPUT")"
eq "multi-line output: delimiter not inside the value" 1 "$(grep -c -x -F -- "$DELIM" "$GITHUB_OUTPUT")"
eq "multi-line output: value intact" "line1 MOTE_EOF_ line3" "$(sed -n '2,4p' "$GITHUB_OUTPUT" | tr '\n' ' ' | sed 's/ $//')"

# Real transports against stand-ins on PATH (no dry-run): the token must never be in argv.
reset lib-transport
mkdir -p "$CASE/bin"
cat >"$CASE/bin/curl" <<'FAKE_CURL'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$FAKE_DIR/curl.argv"
cat >"$FAKE_DIR/curl.stdin"
out=""
while [ $# -gt 0 ]; do
  case "$1" in -o) out=$2; shift ;; esac
  shift
done
printf '{"id": 9}' >"$out"
printf 201
FAKE_CURL
cat >"$CASE/bin/gh" <<'FAKE_GH'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$FAKE_DIR/gh.argv"
printf '%s|%s' "${GH_HOST:-}" "${GH_ENTERPRISE_TOKEN:-}" >"$FAKE_DIR/gh.env"
printf '{"login": "x"}'
FAKE_GH
chmod +x "$CASE/bin/curl" "$CASE/bin/gh"
transport() { # TRANSPORT SERVER_URL -> runs `api POST repos/acme/widgets/issues/7/comments body`
  printf '{"body": "hi"}' >"$CASE/body.json"
  env -u GH_HOST -u GH_ENTERPRISE_TOKEN PATH="$CASE/bin:$PATH" FAKE_DIR="$CASE" MOTE_DRY_RUN=0 \
    MOTE_API_TRANSPORT="$1" GITHUB_SERVER_URL="$2" GITHUB_API_URL="${3:-https://api.github.com}" \
    GH_TOKEN='s3cr3t"tok' bash -c '. "$1/lib.sh"; api POST repos/acme/widgets/issues/7/comments "$2" && echo "$API_STATUS"' \
    _ "$SCRIPTS" "$CASE/body.json"
}
eq "curl transport: 2xx status returned" 201 "$(transport curl https://github.com)"
hasnt "curl transport: token not in argv" "s3cr3t" "$CASE/curl.argv"
has "curl transport: config read from stdin" "--config" "$CASE/curl.argv"
eq "curl transport: Authorization header on stdin, quotes escaped" 'header = "Authorization: Bearer s3cr3t\"tok"' \
  "$(cat "$CASE/curl.stdin")"
has "curl transport: API URL" "https://api.github.com/repos/acme/widgets/issues/7/comments" "$CASE/curl.argv"
transport curl https://ghes.example.com https://ghes.example.com/api/v3/ >/dev/null
has "curl transport on GHES: GITHUB_API_URL (trailing slash handled)" \
  "https://ghes.example.com/api/v3/repos/acme/widgets/issues/7/comments" "$CASE/curl.argv"
eq "gh transport: works" 200 "$(transport gh https://github.com)"
hasnt "gh transport: token not in argv" "s3cr3t" "$CASE/gh.argv"
transport gh https://ghes.example.com >/dev/null
eq "gh transport on GHES: gh gets GH_HOST and GH_ENTERPRISE_TOKEN" 'ghes.example.com|s3cr3t"tok' "$(cat "$CASE/gh.env")"

# =================================================================================================
echo "action.yml wiring"
A="$SCRIPTS/../action.yml"
step_line() { grep -n -- "- name: $1\$" "$A" | head -n 1 | cut -d: -f1; }
step() { # NAME -> that step's lines
  awk -v n="    - name: $1" '$0 == n { p = 1; print; next } p && /^    - name: / { exit } p' "$A"
}
eq "cache is restored after cache-scope is computed" "true" \
  "$([ "$(step_line 'Resolve new-code base')" -lt "$(step_line 'Restore analysis cache')" ] && echo true || echo false)"
eq "restore: path, key and restore prefix are scoped" 3 \
  "$(step 'Restore analysis cache' | grep -c -F 'steps.prepare.outputs.cache-scope }}')"
eq "save: path and key are scoped" 2 "$(step 'Save analysis cache' | grep -c -F 'steps.prepare.outputs.cache-scope }}')"
eq "scan uses the scoped cache dir" 1 "$(step 'Scan' | grep -c -F 'mote-cache-${{ steps.prepare.outputs.cache-scope }}')"
eq "check run knows whether the gate is enforced" 1 \
  "$(step 'Publish check run' | grep -c -F 'INPUT_FAIL_ON_GATE: ${{ inputs.fail-on-gate }}')"
eq "PR comment only in scan mode, keyed by path" 2 \
  "$(step 'Update PR comment' | grep -c -F -e "if: inputs.mode == 'scan'" -e 'INPUT_PATH: ${{ inputs.path }}')"
eq "override also runs for a failed gate in mode: gate" 1 \
  "$(step 'Check override label' | grep -c -F "(inputs.mode == 'gate' && steps.gate.outputs.is-pr == 'true' && steps.gate.outputs.exit-code == '1')")"
eq "no \${{ }} inside run: scripts" 0 "$(grep -c '^ *run:.*\${{' "$A" || true)"

# =================================================================================================
echo
echo "passed: $PASS, failed: $FAIL"
[ "$FAIL" -eq 0 ]
