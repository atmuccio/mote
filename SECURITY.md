# Security policy

## Supported versions

Mote is in its 0.x series. Security fixes are made for the latest release only; upgrade to it
before reporting.

| Version | Supported |
|---|---|
| Latest 0.x release | Yes |
| Older releases | No |

## Reporting a vulnerability

Please report vulnerabilities **privately**, through GitHub's private vulnerability reporting:
open the repository's **Security** tab and choose **Report a vulnerability**
([direct link](https://github.com/atmuccio/mote/security/advisories/new)). Don't open a public
issue, pull request or discussion for a vulnerability.

Include what you can of:

* the Mote version (`mote version`) and your operating system;
* the component: the `mote` binary, the GitHub Action (`action.yml` and its scripts), or the
  release artifacts;
* a description of the problem and its impact, and the steps or files that reproduce it.

Reports are handled on a best-effort basis. You'll be kept informed in the advisory; the fix
ships in a new release, and the advisory is published once that release is available. Credit is
given in the advisory unless you prefer otherwise.

## Scope

In scope, for example:

* the binary running anything from a repository it scans (Mote reads repositories as data and
  must never execute code, build tools or configuration scripts from them);
* the binary writing files outside its cache directory and the output paths it was given;
* the binary making network connections other than the `git` clone that `--remote` asks for;
* crafted repository content (file names, file contents, configuration, git metadata) that
  makes the GitHub Action leak its token, issue workflow commands, or act on input it should
  treat as untrusted;
* release artifacts that don't match their published checksums.

Out of scope:

* problems Mote fails to find in *your* code: Mote is a code-quality tool, not a security scanner
  (missed findings are welcome as regular issues);
* vulnerabilities in dependencies that are already public, unless Mote is affected in a way that
  needs a change in Mote (please report those upstream as well);
* a scan that is slow or uses a lot of memory on an extremely large input, unless a small input
  causes it.
