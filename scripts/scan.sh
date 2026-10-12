#!/usr/bin/env bash
# Run `mote scan` and record its exit code without failing the step (the gate is enforced by
# finalize.sh after everything has been published).
#
# Inputs (env): MOTE_BIN, INPUT_PATH, INPUT_CONFIG, BASE_SHA, INPUT_SARIF_MIN_SEVERITY,
#               INPUT_EXTRA_ARGS, MOTE_OUT_DIR, MOTE_CACHE_DIR
# Outputs: exit-code, gate (passed|failed|error), new-findings, results-json, sarif, markdown, checks
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"
require_cmd jq xargs

BIN=${MOTE_BIN:-mote}
OUT_NATIVE=${MOTE_OUT_DIR:?MOTE_OUT_DIR is required}
OUT=$(to_unix_path "$OUT_NATIVE")
CACHE_DIR=$(to_unix_path "${MOTE_CACHE_DIR:-}")
mkdir -p "$OUT"
rm -f "$OUT/results.json" "$OUT/results.sarif" "$OUT/summary.md" "$OUT/checks.json"

# Paths are operands: a leading '-' must not turn them into options (e.g. a path named --no-gate).
args=(scan "$(safe_path_arg "${INPUT_PATH:-.}")"
  --json "$OUT/results.json"
  --sarif "$OUT/results.sarif"
  --sarif-min-severity "${INPUT_SARIF_MIN_SEVERITY:-MEDIUM}"
  --markdown "$OUT/summary.md"
  --github-checks "$OUT/checks.json")
if [ -n "${INPUT_CONFIG:-}" ]; then args+=(--config "$(safe_path_arg "$INPUT_CONFIG")"); fi
if [ -n "${BASE_SHA:-}" ]; then args+=(--base "$BASE_SHA"); fi
if [ -n "$CACHE_DIR" ]; then args+=(--cache-dir "$CACHE_DIR"); fi

# extra-args: split like a shell would (quotes honored) but never evaluated. A quoting error fails
# the step instead of silently scanning without the user's arguments.
extra=()
case "${INPUT_EXTRA_ARGS:-}" in *[![:space:]]*)
  split=$(mktemp_file)
  if ! printf '%s' "$INPUT_EXTRA_ARGS" | xargs printf '%s\n' >"$split" 2>"$split.err"; then
    die "Could not parse extra-args (unbalanced quotes?): $(head -c 500 "$split.err")"
  fi
  while IFS= read -r a; do extra+=("$a"); done <"$split"
  rm -f "$split" "$split.err"
  ;;
esac
case " ${INPUT_EXTRA_ARGS:-} " in
  *" --color"*) ;;
  *) args+=(--color always) ;; # the Actions log renders ANSI colors
esac

log "+ $BIN ${args[*]} ${extra[*]+${extra[*]}}"
# mote prints file paths and messages from the code under review: fence them so a crafted file
# name can't issue workflow commands. stderr joins stdout to stay inside the fence.
stop_commands
set +e
"$BIN" "${args[@]}" ${extra[@]+"${extra[@]}"} 2>&1
rc=$?
set -e
resume_commands

case "$rc" in
  0) gate=passed ;;
  1) gate=failed ;;
  *) gate=error ;;
esac

new_findings=""
if [ -s "$OUT/results.json" ]; then
  # With --base: findings whose new-code status is "new"; without: every unsuppressed finding.
  new_findings=$(jq -r --arg base "${BASE_SHA:-}" '
    [ .findings[]? | select(.suppressed == null)
      | select(if $base == "" then true else (.status // "") == "new" end) ] | length
  ' "$OUT/results.json" 2>/dev/null || true)
fi

case "$rc" in
  0) log "mote: quality gate passed" ;;
  1) log "mote: quality gate FAILED" ;;
  2) error "mote: usage or configuration error (exit 2)" ;;
  *) error "mote: the scan could not complete (exit $rc)" ;;
esac

file_out() { # NAME FILE — the output is the runner-native path, empty if the file wasn't written
  if [ -s "$OUT/$2" ]; then set_output "$1" "$OUT_NATIVE/$2"; else set_output "$1" ""; fi
}
set_output exit-code "$rc"
set_output gate "$gate"
set_output new-findings "$new_findings"
file_out results-json results.json
file_out sarif results.sarif
file_out markdown summary.md
file_out checks checks.json
