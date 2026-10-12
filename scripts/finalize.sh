#!/usr/bin/env bash
# Turn the scan result into the step's exit status — the last step of the action, so annotations,
# summary, comment, artifact and cache are all published before the job goes red.
#
#   exit 0  gate passed
#   exit 1  gate failed, fail-on-gate is true, and no honored override
#   exit 2/3 (or anything else) from mote is always propagated: a scan that could not run is
#           never green, whatever fail-on-gate or the override label say.
#
# Inputs (env): MOTE_EXIT_CODE, INPUT_FAIL_ON_GATE, OVERRIDE, OVERRIDE_ACTOR, INPUT_OVERRIDE_LABEL
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
. "$SCRIPT_DIR/lib.sh"

rc=${MOTE_EXIT_CODE:-}
case "$rc" in
  0)
    log "Mote quality gate passed."
    exit 0
    ;;
  1)
    if [ "${OVERRIDE:-none}" = honored ]; then
      notice "Mote quality gate failed but was overridden by @${OVERRIDE_ACTOR:-?} (label '${INPUT_OVERRIDE_LABEL:-mote-override}')."
      exit 0
    fi
    if is_true "${INPUT_FAIL_ON_GATE:-true}"; then
      error "Mote quality gate failed. See the job summary, the check run annotations, or the PR comment."
      exit 1
    fi
    warn "Mote quality gate failed (not enforced: fail-on-gate is false)."
    exit 0
    ;;
  '')
    error "mote did not run."
    exit 1
    ;;
  *[!0-9]*)
    error "Unexpected mote exit code '$rc'."
    exit 1
    ;;
  *)
    error "mote failed with exit code $rc (2 = usage/configuration error, 3 = internal error)."
    # `exit 256` (or 00) would be green: anything outside 2..255 fails with 1.
    rc=$((10#$rc)) 2>/dev/null || rc=1
    if [ "$rc" -lt 2 ] || [ "$rc" -gt 255 ]; then rc=1; fi
    exit "$rc"
    ;;
esac
