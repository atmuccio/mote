#!/usr/bin/env bash
# Append the Mote report to the job summary ($GITHUB_STEP_SUMMARY, 1 MiB per step).
#
# Inputs (env): MOTE_MARKDOWN_FILE, MOTE_OVERRIDE_MD, MOTE_EXIT_CODE, MOTE_VERSION,
#               MOTE_ARTIFACT_NAME, GITHUB_STEP_SUMMARY
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"

MAX_BYTES=1000000 # a little under the 1 MiB step limit

MOTE_MARKDOWN_FILE=$(to_unix_path "${MOTE_MARKDOWN_FILE:-}")
MOTE_OVERRIDE_MD=$(to_unix_path "${MOTE_OVERRIDE_MD:-}")
export MOTE_MARKDOWN_FILE MOTE_OVERRIDE_MD

report=$(mktemp_file)
compose_report "$report" "$MAX_BYTES"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$report" >>"$(to_unix_path "$GITHUB_STEP_SUMMARY")"
  log "Job summary written ($(wc -c <"$report" | tr -d ' ') bytes)."
else
  cat "$report"
fi
