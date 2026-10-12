# Contributing to Mote

Thank you for taking the time to help. The most useful contributions right now are **issues**:
bug reports, false positives, missed problems and feature requests. **Pull requests are not
accepted for now** (see [below](#pull-requests) for why).

Before opening an issue, please check that you are on the latest release (`mote version`) and
search the [existing issues](https://github.com/atmuccio/mote/issues). Then pick a template on the
[new issue](https://github.com/atmuccio/mote/issues/new/choose) page.

To report a security vulnerability in Mote itself, don't open an issue: follow
[SECURITY.md](SECURITY.md).

## False positives and missed problems

A **false positive** is a finding on code that is fine. Mote's rules favor precision: a rule that
cannot be sure is supposed to stay silent, so every false positive is a bug worth reporting. A
**false negative** is a problem a rule should have reported but didn't.

A good report has:

* **The rule id**, for example `java/string-reference-equality` (for a false negative: the rule you
  expected, or "none" if no rule covers it).
* **The Mote version**: the output of `mote version`.
* **The language**: Java (and its release, if the rule depends on it: `mote config explain <file>`
  prints the one Mote used), JavaScript, TypeScript, JSX or TSX.
* **A minimal code snippet** that reproduces it: the smallest self-contained file you can make,
  ideally a few lines. Mote analyzes one file at a time (duplication aside), so a single file is
  almost always enough.
* **What you expected**: why the code is fine (or why it is wrong), in a sentence or two.
* **The finding as printed** (`mote scan --format jsonl` gives one line per finding) and any
  relevant part of `.mote.yaml`.

Please share only code you are allowed to share: reduce proprietary code to a neutral example
that shows the same pattern.

While you wait for a fix, silence a false positive with a suppression that names the rule and
says why (`// mote-ignore[java/rule-id]: reason`), or record it in the triage file
(`mote triage add <finding-id> --status false-positive --reason "…" --results results.json`); see
[docs/cli.md](docs/cli.md).

## Bugs

For crashes, wrong exit codes, wrong new-code classification, configuration problems or slow
scans, include the Mote version, your operating system, the exact command line, what happened
(the output, with `-v` for more detail) and what you expected. A public repository and commit that
reproduce the problem are ideal; `mote scan --timings` helps with performance reports.

## Feature requests

New rules, rule changes, options, output formats and integrations are all welcome as ideas.
For a rule, describe the problem pattern **in your own words**, with an example that should be
reported and a similar one that should not. Please don't paste rule descriptions or examples
from other tools' documentation: Mote's rules are written from scratch, and their provenance
matters (see [Provenance](README.md#provenance)). Naming a permissively licensed tool that has a
similar rule is fine.

## Pull requests

Pull requests are not accepted for now, and will be closed with a pointer to this file. The
reasons:

* **Provenance.** Every rule, description and fixture in Mote is original work with a documented
  origin. Reviewing outside code to that standard takes more time than the project can give
  at this stage.
* **How the code is published.** Development happens in a separate tree, and each release
  reaches this repository as one commit. A pull request could not be merged here as is.
* **Stability.** The internal interfaces (the plugin contract, the rule APIs) are still changing
  between 0.x releases.

This may change. If it does, this file will say so and describe how contributions are licensed
(they would be dual-licensed under MIT OR Apache-2.0, like the rest of Mote). Until then, an issue
with a clear description and a minimal reproduction is the best way to get something fixed or
added.

## License

Mote is licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option. Code snippets in issues are used to reproduce and
fix the problem, and may be turned into test fixtures in Mote's own words.
