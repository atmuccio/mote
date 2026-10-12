# Mote in other CI systems

Recipes for GitLab CI, Jenkins, Azure Pipelines and Bitbucket Pipelines (GitHub has its own action:
[github-action.md](github-action.md)). The pattern is the same everywhere:

1. Install the `mote` binary in the job (see [Install](../README.md#install)); it needs the
   git history back to the merge base for new-code analysis, so fetch deeply.
2. Run **one** `mote scan` that writes every report the CI service reads, as side files.
3. Let the CI service pick the reports up. **Mote never posts anything**: the binary never
   touches the network; comments and uploads are made by the CI job with the service's own tools.
4. The job's result is the quality gate: `mote scan` exits 1 when the gate fails (0 passed,
   2 usage/config error, 3 internal error; [cli.md](cli.md#exit-codes)). Publish the reports even
   when the step fails.

| Report | Flag | Read by |
|---|---|---|
| GitLab Code Quality | `--gitlab-codequality FILE` | GitLab `artifacts:reports:codequality` |
| Checkstyle XML | `--checkstyle FILE` | Jenkins Warnings NG (`checkStyle` tool), other dashboards |
| JUnit XML | `--junit FILE` | GitLab `artifacts:reports:junit`, Jenkins `junit`, Azure `PublishTestResults@2` |
| Markdown summary | `--markdown FILE` | a merge-request note posted by the job, the Azure build summary |
| HTML report | `--html FILE` | a build artifact anyone can open in a browser (self-contained, no network) |
| Results JSON | `--json FILE` | `mote gate --results` in a later job (e.g. after coverage), `mote compare`, `--baseline` |
| Bitbucket Code Insights | `--bitbucket-insights DIR` | the Bitbucket Code Insights API (uploaded by the job) |
| Summary record | `--summary-json FILE` | your trend archive (one line of JSON per run) |

On a merge/pull request, pass `--base <target branch>` so findings are classified as new or
unchanged and the gate applies to new code. On the default branch, scan without `--base` and
without failing the job (`--no-gate`): there every finding counts, and the run exists to produce
the baseline reports the CI service compares merge requests with.

## GitLab CI

GitLab compares the merge request's Code Quality and JUnit reports with those of the target
branch's latest pipeline, so run the job on the default branch too.

```yaml
mote:
  stage: test
  variables:
    GIT_DEPTH: "0"                      # new-code analysis needs the merge base
  script:
    - |
      if [ -n "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME" ]; then
        git fetch --quiet origin "$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
        set -- --base "origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME"
      else
        set -- --no-gate                # default branch: produce the baseline, don't fail on old debt
      fi
      mote scan "$@" \
        --gitlab-codequality gl-code-quality-report.json \
        --junit mote-junit.xml \
        --markdown mote-summary.md \
        --html mote-report.html \
        --json mote-results.json
  artifacts:
    when: always                        # publish the reports when the gate fails too
    reports:
      codequality: gl-code-quality-report.json
      junit: mote-junit.xml
    paths: [mote-report.html, mote-summary.md, mote-results.json]
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

* The Code Quality report lists every unsuppressed finding; GitLab shows what the merge request
  introduces and resolves by comparing fingerprints (they survive line shifts and usually renames; see
  [cli.md](cli.md#output-formats)). Where the findings appear (widget, diff) depends on your
  GitLab tier.
* The JUnit report shows the gate as a test suite (one test case per condition) and one test
  suite per rule; add `--new-only` to list only new findings there.
* The HTML report is a single file: open it from the job's artifacts (**Browse**), or publish it
  with GitLab Pages.

**Markdown summary as a merge-request note.** A later job posts it with the GitLab API. The
default `CI_JOB_TOKEN` cannot write notes: create a project access token with the `api` scope and
store it as a masked CI/CD variable (`MOTE_NOTE_TOKEN` here). This keeps one note per merge
request, edited in place, found by the marker line the summary starts with:

```yaml
mote-note:
  stage: .post
  needs: [{ job: mote, artifacts: true }]
  when: always
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  script:
    - |
      api="$CI_API_V4_URL/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"
      id=$(curl --fail -sS --header "PRIVATE-TOKEN: $MOTE_NOTE_TOKEN" "$api?per_page=100" |
        jq -r '[.[] | select(.body | startswith("<!-- mote:summary -->"))][0].id // empty')
      if [ -n "$id" ]; then
        curl --fail -sS -X PUT --header "PRIVATE-TOKEN: $MOTE_NOTE_TOKEN" \
          --data-urlencode "body@mote-summary.md" "$api/$id" >/dev/null
      else
        curl --fail -sS -X POST --header "PRIVATE-TOKEN: $MOTE_NOTE_TOKEN" \
          --data-urlencode "body@mote-summary.md" "$api" >/dev/null
      fi
```

The image needs `curl` and `jq`. The summary is GitHub-flavored Markdown; GitLab renders its
tables, and emoji shortcodes such as `:x:` too.

## Jenkins

With the [Warnings Next Generation](https://plugins.jenkins.io/warnings-ng/) plugin (Checkstyle
parser) and the JUnit plugin. In a multibranch pipeline, `CHANGE_TARGET` is set on pull/merge
request builds:

```groovy
pipeline {
  agent any
  stages {
    stage('Mote') {
      steps {
        sh '''
          if [ -n "$CHANGE_TARGET" ]; then
            git fetch --quiet origin "$CHANGE_TARGET"
            set -- --base "origin/$CHANGE_TARGET"
          else
            set -- --no-gate
          fi
          mote scan "$@" \
            --checkstyle mote-checkstyle.xml \
            --junit mote-junit.xml \
            --html mote-report.html \
            --json mote-results.json
        '''
      }
      post {
        always {
          recordIssues(tools: [checkStyle(pattern: 'mote-checkstyle.xml', id: 'mote', name: 'Mote')])
          junit(testResults: 'mote-junit.xml', allowEmptyResults: true, skipMarkingBuildUnstable: true)
          archiveArtifacts(artifacts: 'mote-*', allowEmptyArchive: true)
        }
      }
    }
  }
}
```

* The `sh` step fails when the gate fails; `post { always { … } }` still records the reports.
* Warnings NG compares each build's issues with a reference build to show new and fixed ones, so
  the Checkstyle report always lists every unsuppressed finding (HIGH → error, MEDIUM → warning,
  LOW → info; the rule id is the issue type).
* Every finding is a failed test case in the JUnit report; `skipMarkingBuildUnstable` keeps them
  from turning every build unstable (the gate already decides the result). Drop it, or add
  `--new-only`, if you want the test trend to track findings.
* The HTML report is archived with the other `mote-*` files; open it from the build's
  artifacts.
* The checkout must have the target branch's history (shallow clones: fetch deeper).

## Azure Pipelines

```yaml
steps:
  - checkout: self
    fetchDepth: 0                       # new-code analysis needs the merge base
  - script: |
      if [ -n "$SYSTEM_PULLREQUEST_TARGETBRANCH" ]; then
        target=${SYSTEM_PULLREQUEST_TARGETBRANCH#refs/heads/}
        git fetch --quiet origin "$target"
        set -- --base "origin/$target"
      else
        set -- --no-gate
      fi
      mote scan "$@" \
        --junit "$(Build.ArtifactStagingDirectory)/mote-junit.xml" \
        --markdown "$(Build.ArtifactStagingDirectory)/mote-summary.md" \
        --html "$(Build.ArtifactStagingDirectory)/mote-report.html" \
        --json "$(Build.ArtifactStagingDirectory)/mote-results.json"
    displayName: Mote scan
  - task: PublishTestResults@2
    condition: succeededOrFailed()
    inputs:
      testResultsFormat: JUnit
      testResultsFiles: $(Build.ArtifactStagingDirectory)/mote-junit.xml
      testRunTitle: Mote
  - script: echo "##vso[task.uploadsummary]$(Build.ArtifactStagingDirectory)/mote-summary.md"
    condition: succeededOrFailed()
    displayName: Mote summary
  - publish: $(Build.ArtifactStagingDirectory)
    artifact: mote
    condition: succeededOrFailed()
```

* The scan step fails when the gate fails; the publishing steps run anyway
  (`succeededOrFailed()`).
* `PublishTestResults@2` shows the gate conditions and the findings in the **Tests** tab. It does
  not fail the pipeline on failed tests unless `failTaskOnFailedTests: true` is set; the gate is
  the scan step's exit code.
* `##vso[task.uploadsummary]` adds the Markdown summary to the run's summary page. It is
  GitHub-flavored Markdown: tables render, emoji shortcodes such as `:x:` may show as text.
* The HTML report is in the `mote` pipeline artifact; download it and open it in a browser.

## Bitbucket Pipelines

Bitbucket shows a commit's (and a pull request's) **Code Insights**: a report with a verdict and
a few numbers, and annotations on the changed lines. `--bitbucket-insights DIR` writes both as
files; the step uploads them through the Pipelines proxy, which authenticates the request for
the repository being built, so no token is needed:

```yaml
image: atlassian/default-image:4        # has curl, jq and git; install mote in the step

definitions:
  steps:
    - step: &mote
        name: Mote
        clone:
          depth: full                   # new-code analysis needs the merge base
        script:
          - |
            if [ -n "$BITBUCKET_PR_DESTINATION_BRANCH" ]; then
              git fetch --quiet origin "$BITBUCKET_PR_DESTINATION_BRANCH"
              set -- --base "origin/$BITBUCKET_PR_DESTINATION_BRANCH"
            else
              set -- --no-gate            # default branch: produce the baseline, don't fail on old debt
            fi
            mote scan "$@" \
              --bitbucket-insights mote-insights \
              --junit test-results/mote-junit.xml \
              --html mote-report.html \
              --json mote-results.json \
              --summary-json mote-summary.json
        after-script:                   # runs when the gate failed too
          - |
            api="http://api.bitbucket.org/2.0/repositories/$BITBUCKET_WORKSPACE/$BITBUCKET_REPO_SLUG/commit/$BITBUCKET_COMMIT/reports/mote"
            proxy="--proxy http://localhost:29418"
            [ -f mote-insights/report.json ] || exit 0
            curl -sS $proxy -X DELETE "$api" >/dev/null || true      # a rerun starts clean
            curl --fail -sS $proxy -X PUT "$api" \
              -H 'Content-Type: application/json' --data-binary @mote-insights/report.json
            jq -c 'range(0; length; 100) as $i | .[$i:$i+100]' mote-insights/annotations.json |
              while read -r chunk; do
                printf '%s' "$chunk" | curl --fail -sS $proxy -X POST "$api/annotations" \
                  -H 'Content-Type: application/json' --data-binary @-
              done
        artifacts:
          - mote-*
          - mote-insights/**

pipelines:
  pull-requests:
    '**':
      - step: *mote
  branches:
    main:
      - step: *mote
```

* The report's result is the quality gate (`PASSED` / `FAILED`) and its details name the failed
  conditions; its data shows new findings, the grades, duplication and coverage (on new code
  when there is a base), estimated effort and lines of code.
* Annotations cover the new and worsened findings on pull requests (every finding on the
  default branch), at most 1,000 per report — the most severe are kept. The API takes 100 per
  request, hence the `jq` split. Reliability findings are `BUG`s, others `CODE_SMELL`s; HIGH,
  MEDIUM and LOW severities carry over.
* The proxy only serves `http://api.bitbucket.org` (plain `http`, the proxy adds TLS and
  authentication). Outside Pipelines, send the same files to `https://api.bitbucket.org` with an
  access token.
* Pipelines picks up JUnit XML from a `test-results` directory: the gate conditions and findings
  show in the step's **Tests** tab. The HTML report is in the step's artifacts.
* The files follow Bitbucket Cloud's Code Insights API. Bitbucket Data Center has its own
  Insights API with different field names; transform the files with `jq` for it.

## Trends: archive a record per run, compare with the previous run

Each run on the default branch can leave two things behind: the results JSON (`--json`), which
`mote compare` reads, and a one-line metrics record (`--summary-json`) for charts. The
binary only writes files; keeping them is the CI job's business.

1. **Archive the records.** Keep every `mote-summary.json` (build artifacts with a long
   expiry, a bucket, or a line appended to a `history.jsonl` on a dedicated branch). A trend
   table is one `jq` away:

   ```sh
   jq -r '[.date, .revision, .measures.ncloc, .findings.high, .findings.medium, .findings.low,
           .grades.maintainability.grade, .grades.reliability.grade, .measures.effort_minutes.total]
          | @csv' history.jsonl > trend.csv
   ```

2. **Compare with the previous run.** Fetch the previous default-branch results and put the
   differences in the job summary. On GitLab, the latest successful job's artifacts are one API
   call away (the job token may read its own project's artifacts):

   ```yaml
   mote-trend:
     stage: .post
     needs: [{ job: mote, artifacts: true }]
     rules:
       - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
     script:
       - |
         curl --fail -sS -o previous.json --header "JOB-TOKEN: $CI_JOB_TOKEN" \
           "$CI_API_V4_URL/projects/$CI_PROJECT_ID/jobs/artifacts/$CI_DEFAULT_BRANCH/raw/mote-results.json?job=mote" \
           || { echo "no previous results yet"; exit 0; }
         mote compare previous.json mote-results.json --format markdown -o mote-compare.md
         mote compare previous.json mote-results.json      # readable in the job log
     artifacts:
       paths: [mote-compare.md]
   ```

   Elsewhere, restore the previous `mote-results.json` from the CI cache (keyed by branch)
   or download the last build's artifact, then run the same `mote compare`.

* `compare` pairs findings by fingerprint, so moved lines and renamed files are not reported as
  fixed and added. It warns when the two scans used different engine versions, rule sets or
  grade bands: after an upgrade or a configuration change, added and fixed findings may come
  from the rules rather than from the code.
* For a first rollout on a code base with old findings, `--baseline` turns such a saved results
  file into the reference for the gate: `mote scan --baseline previous.json` fails only on
  findings the previous run did not have ([cli.md](cli.md#baselines)).
