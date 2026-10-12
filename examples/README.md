# Examples

Ready-to-copy setups. All of them use the published release binaries (checked against their
SHA-256 before use) and pin an exact version: 0.x releases may still change rules and defaults.

| File | What it does |
|---|---|
| [`workflows/mote.yml`](workflows/mote.yml) | GitHub Actions pull-request gate: check run with inline annotations, sticky PR comment, SARIF upload to code scanning, the gate as a required check |
| [`workflows/mote-with-coverage.yml`](workflows/mote-with-coverage.yml) | The same, plus coverage on new code from your test job's reports |
| [`gitlab/.gitlab-ci.yml`](gitlab/.gitlab-ci.yml) | GitLab CI merge-request gate feeding the Code Quality widget and the JUnit test view |
| [`ci/mote-ci.sh`](ci/mote-ci.sh) | One step for any CI service: finds the target branch from the CI's variables, gates new code, writes SARIF, JUnit, Checkstyle, HTML, Markdown and JSON reports |
| [`ci/install-mote.sh`](ci/install-mote.sh) | Downloads and verifies a release binary for the current machine (also from an internal mirror) |
| [`hooks/pre-commit`](hooks/pre-commit) | Git pre-commit hook: blocks a commit only when the staged changes add findings that fail the gate |
| [`hooks/pre-commit-config.yaml`](hooks/pre-commit-config.yaml) | The same check as a hook of the pre-commit framework |

More: [docs/github-action.md](../docs/github-action.md) (every input of the action),
[docs/ci.md](../docs/ci.md) (Jenkins, Azure Pipelines, Bitbucket Pipelines, merge-request notes,
trends) and [docs/cli.md](../docs/cli.md) (every option and output format).
