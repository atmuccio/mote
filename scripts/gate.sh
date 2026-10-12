#!/usr/bin/env bash
# `mode: gate` — re-evaluate the quality gate from a saved results file with `mote gate`, e.g. in
# a job that runs after the tests to add the coverage condition. Like scan.sh it only records the
# exit code; override.sh may waive a failure and finalize.sh enforces it.
#
# `--only <conditions>` (default `coverage`; comma-separated) limits the verdict, so a job that only
# adds coverage doesn't fail again on the severity findings the scan job already reported (and that
# an override label may have waived there).
#
# Inputs (env): MOTE_BIN, INPUT_RESULTS, INPUT_COVERAGE (newline-separated globs),
#               INPUT_GATE_ONLY, INPUT_CONFIG, INPUT_PATH, MOTE_OUT_DIR, GITHUB_*
# Outputs: exit-code, gate (passed|failed|error), markdown, is-pr, pr-number, head-sha
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd jq

BIN=${MOTE_BIN:-mote}
OUT_NATIVE=${MOTE_OUT_DIR:?MOTE_OUT_DIR is required}
OUT=$(to_unix_path "$OUT_NATIVE")
mkdir -p "$OUT"
rm -f "$OUT/gate.md"

trim() { printf '%s' "$1" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'; }

# --- context (prepare.sh doesn't run in this mode: the job may have no full checkout) --------------
IS_PR=false
PR_NUMBER=""
HEAD_SHA=${GITHUB_SHA:-}
if is_pr_event; then
  IS_PR=true
  PR_NUMBER=$(event_get .pull_request.number)
  case "$PR_NUMBER" in *[!0-9]*) die "Unexpected pull request number '$PR_NUMBER' in the event." ;; esac
  HEAD_SHA=$(event_get .pull_request.head.sha)
fi
set_output is-pr "$IS_PR"
set_output pr-number "$PR_NUMBER"
set_output head-sha "$HEAD_SHA"

finish() { # EXIT_CODE
  local gate
  case "$1" in 0) gate=passed ;; 1) gate=failed ;; *) gate=error ;; esac
  set_output exit-code "$1"
  set_output gate "$gate"
  if [ -s "$OUT/gate.md" ]; then set_output markdown "$OUT_NATIVE/gate.md"; else set_output markdown ""; fi
  exit 0
}

# --- arguments ----------------------------------------------------------------------------------
RESULTS=$(trim "${INPUT_RESULTS:-}")
[ -n "$RESULTS" ] || RESULTS=mote-results/results.json
if [ ! -f "$(to_unix_path "$RESULTS")" ]; then
  error "Results file '$RESULTS' not found. Download the scan job's results artifact first (actions/download-artifact, name 'mote-results' by default) and point 'results' at its results.json."
  finish 2
fi

args=(gate --results "$(safe_path_arg "$RESULTS")" --format markdown)
ONLY=$(printf '%s' "${INPUT_GATE_ONLY:-}" | tr -d '[:space:]') # 'coverage, worsened' -> coverage,worsened
if [ -n "$ONLY" ]; then args+=(--only "$ONLY"); fi

CONFIG=$(trim "${INPUT_CONFIG:-}")
SRC=$(normalize_rel_path "${INPUT_PATH:-.}")
if [ -z "$CONFIG" ] && [ "$SRC" != . ] && [ -f "$(to_unix_path "$SRC")/.mote.yaml" ]; then
  CONFIG="$SRC/.mote.yaml" # mote gate defaults to ./.mote.yaml
fi
if [ -n "$CONFIG" ]; then args+=(--config "$(safe_path_arg "$CONFIG")"); fi

n_cov=0
while IFS= read -r line || [ -n "$line" ]; do
  line=$(trim "$line")
  [ -n "$line" ] || continue
  args+=(--coverage "$line")
  n_cov=$((n_cov + 1))
done <<EOF
${INPUT_COVERAGE:-}
EOF
if [ "$n_cov" -eq 0 ] && [ "$ONLY" = coverage ]; then
  warn "No 'coverage' globs given: the coverage condition is evaluated without coverage reports."
fi

# --- run ----------------------------------------------------------------------------------------
log "+ $BIN ${args[*]}"
set +e
stop_commands
# shellcheck disable=SC2069 # on purpose: stderr to the log (inside the fence), stdout to the file
"$BIN" "${args[@]}" 2>&1 >"$OUT/gate.md"
rc=$?
if [ -s "$OUT/gate.md" ]; then cat "$OUT/gate.md"; fi
resume_commands
set -e

case "$rc" in
  0) log "mote gate: passed" ;;
  1) log "mote gate: FAILED" ;;
  2) error "mote gate: usage or configuration error (exit 2)" ;;
  *) error "mote gate: could not complete (exit $rc)" ;;
esac
finish "$rc"
