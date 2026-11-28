# Security Policy

## Supported Versions

Security fixes land on the latest `0.37.x` and subsequent `0.x` release line on `main`. Earlier tags do not receive backported patches.

## Security Model

Catchledger makes zero network calls in production code or tests and reads only local CI workflow-run exports, Git history exports or local clones (`--git-repo` invoking local `git log` without network transport), `canaryfield` score documents, and token-usage documents. Its pure-Rust JSON reader refuses duplicate object keys at any nesting depth and enforces a `4000` nesting ceiling, and its fixed-point `DecimalValue` arithmetic computes flake rates, runtime costs, and cost-per-catch figures deterministically without binary floating-point rounding drift. Never commit real CI logs, internal repository paths, usernames, or private token transcripts into test fixtures.

## Reporting a Vulnerability

Do not open a public issue for a suspected security vulnerability. Report vulnerabilities privately through [GitHub Security Advisories](https://github.com/david-engelmann/catchledger/security/advisories/new) with synthetic inputs only, naming the affected version and the relevant section of `CONTRACT.md`. Every report receives an initial review within seven days, and confirmed fixes ship with a regression pin in the test suite before public disclosure.
