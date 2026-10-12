#!/bin/sh
# One Mote step for any CI service: gate a merge/pull request on its new code, or (on the default
# branch) produce the reports without failing on existing debt.
#
#   MOTE_VERSION=0.1.0 sh mote-ci.sh
#
# The target branch of a merge/pull request is read from the CI's own variables (GitHub Actions,
# GitLab, Jenkins, Azure Pipelines, Bitbucket) or from MOTE_TARGET_BRANCH; without one, the scan
# runs with --no-gate. Reports go to $MOTE_REPORTS (default mote-reports/):
#   mote.sarif           SARIF 2.1.0 (code-scanning dashboards)
#   mote-junit.xml       JUnit XML (test views: the gate and one suite per rule)
#   mote-checkstyle.xml  Checkstyle XML (Jenkins Warnings NG and similar)
#   mote-report.html     self-contained HTML report (open it from the job's artifacts)
#   mote-summary.md      Markdown summary (a merge-request note, a build summary)
#   mote-results.json    full results (`mote gate --results`, `mote compare`, `--baseline`)
# Publish the reports even when this step fails. Exit code = the quality gate: 0 passed, 1 failed,
# 2 usage or configuration error, 3 internal error.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
if ! command -v mote >/dev/null 2>&1; then
  bin=$(sh "$here/install-mote.sh" "${MOTE_BIN_DIR:-.mote-bin}")
  PATH="$(cd "$(dirname "$bin")" && pwd):$PATH"
  export PATH
fi

target=${MOTE_TARGET_BRANCH:-${GITHUB_BASE_REF:-${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-${CHANGE_TARGET:-${SYSTEM_PULLREQUEST_TARGETBRANCH:-${BITBUCKET_PR_DESTINATION_BRANCH:-}}}}}}
target=${target#refs/heads/}

if [ -n "$target" ]; then
  # New-code analysis needs the merge base: fetch the target branch, and the history of a shallow clone.
  refspec="+refs/heads/$target:refs/remotes/origin/$target"
  if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
    git fetch --quiet --no-tags --unshallow origin "$refspec"
  else
    git fetch --quiet --no-tags origin "$refspec"
  fi
  set -- --base "origin/$target"
else
  set -- --no-gate
fi

out=${MOTE_REPORTS:-mote-reports}
mkdir -p "$out"
exec mote scan "$@" \
  --sarif "$out/mote.sarif" \
  --junit "$out/mote-junit.xml" \
  --checkstyle "$out/mote-checkstyle.xml" \
  --html "$out/mote-report.html" \
  --markdown "$out/mote-summary.md" \
  --json "$out/mote-results.json"
